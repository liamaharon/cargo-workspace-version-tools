/// git2 helper functions.
///
/// Heavy inspiration taken from https://github.com/rust-lang/git2-rs/tree/master/examples
use git2::{
    AnnotatedCommit, AutotagOption, BranchType, Commit, FetchOptions, IndexAddOption, ObjectType,
    PushOptions, Reference, Remote, RemoteCallbacks, Repository,
};
use std::{
    fs::File,
    io::{self, Write},
    path::Path,
};

pub fn get_current_branch_name<'a>(repo: &'a Repository) -> Result<String, String> {
    let head = repo.head().expect("Failed to get HEAD");

    if head.is_branch() {
        // If HEAD is pointing to a branch, find its name
        let shorthand = head.shorthand().ok_or_else(|| "Branch name not found")?;
        Ok(repo
            .find_branch(shorthand, BranchType::Local)
            .expect("Just looked up branch - it must exist!")
            .name()
            .unwrap()
            .unwrap()
            .to_owned())
    } else {
        // HEAD is not on a branch
        Err("HEAD is detached or not pointing to a branch".to_owned())
    }
}

pub fn is_working_tree_clean<'a>(repo: &'a Repository) -> bool {
    let statuses = repo
        .statuses(None)
        .expect("Failed to get repository statuses");
    for s in statuses.iter() {
        match s.status() {
            git2::Status::WT_NEW
            | git2::Status::WT_MODIFIED
            | git2::Status::WT_DELETED
            | git2::Status::WT_TYPECHANGE
            | git2::Status::WT_RENAMED => return false,
            _ => {}
        }
    }
    true
}

pub fn do_fetch<'a>(
    repo: &'a Repository,
    refs: &[&str],
    remote: &'a mut Remote,
) -> Result<AnnotatedCommit<'a>, git2::Error> {
    let mut cb = RemoteCallbacks::new();

    // Print out our transfer progress.
    cb.transfer_progress(|stats| {
        if stats.received_objects() == stats.total_objects() {
            print!(
                "Resolving deltas {}/{}\r",
                stats.indexed_deltas(),
                stats.total_deltas()
            );
        } else if stats.total_objects() > 0 {
            print!(
                "Received {}/{} objects ({}) in {} bytes\r",
                stats.received_objects(),
                stats.total_objects(),
                stats.indexed_objects(),
                stats.received_bytes()
            );
        }
        io::stdout().flush().unwrap();
        true
    });

    cb.credentials(|_url, username_from_url, _allowed_types| {
        git2::Cred::ssh_key_from_agent(
            username_from_url
                .expect("Failed to parse username from remote url. Remote must be ssh based."),
        )
    });

    let mut fo = FetchOptions::new();
    fo.remote_callbacks(cb);
    // Don't fetch tags, just the refs
    fo.download_tags(AutotagOption::None);
    remote.fetch(refs, Some(&mut fo), None)?;

    // If there are local objects (we got a thin pack), then tell the user
    // how many objects we saved from having to cross the network.
    let stats = remote.stats();
    if stats.local_objects() > 0 {
        println!(
            "\rReceived {}/{} objects in {} bytes (used {} local \
             objects)",
            stats.indexed_objects(),
            stats.total_objects(),
            stats.received_bytes(),
            stats.local_objects()
        );
    } else if stats.received_bytes() > 0 {
        println!(
            "\rReceived {}/{} objects in {} bytes",
            stats.indexed_objects(),
            stats.total_objects(),
            stats.received_bytes()
        );
    }

    let fetch_head = repo.find_reference("FETCH_HEAD")?;
    Ok(repo.reference_to_annotated_commit(&fetch_head)?)
}

pub fn fast_forward(
    repo: &Repository,
    lb: &mut Reference,
    rc: &AnnotatedCommit,
) -> Result<(), git2::Error> {
    let name = match lb.name() {
        Some(s) => s.to_string(),
        None => String::from_utf8_lossy(lb.name_bytes()).to_string(),
    };
    let msg = format!("Fast-Forward: Setting {} to id: {}", name, rc.id());
    println!("{}", msg);
    lb.set_target(rc.id(), &msg)?;
    repo.set_head(&name)?;
    repo.checkout_head(Some(
        git2::build::CheckoutBuilder::default()
            // For some reason the force is required to make the working directory actually get updated
            // I suspect we should be adding some logic to handle dirty working directory states
            // but this is just an example so maybe not.
            .force(),
    ))?;
    Ok(())
}

pub fn do_fast_forward<'a>(
    repo: &'a Repository,
    remote_branch: &str,
    fetch_commit: AnnotatedCommit<'a>,
) -> Result<(), git2::Error> {
    checkout_local_branch(repo, remote_branch)?;

    // 1. do a merge analysis
    let analysis = repo.merge_analysis(&[&fetch_commit])?;

    // 2. Do the appopriate merge
    if analysis.0.is_fast_forward() {
        println!("Performing fast-forward...");
        let refname = format!("refs/heads/{}", remote_branch);
        match repo.find_reference(&refname) {
            Ok(mut r) => {
                fast_forward(repo, &mut r, &fetch_commit)?;
            }
            Err(e) => {
                return Err(git2::Error::from_str(
                    format!("Failed to find reference {} on remote: {}", refname, e).as_str(),
                ));
            }
        };
    } else if analysis.0.is_normal() {
        return Err(git2::Error::from_str(format!("Unable to automatically fast-forward branch {}. Please sync your local branch with the origin and try again.", remote_branch).as_str()));
    } else {
        println!("Already up to date.");
        checkout_local_branch(repo, remote_branch)?;
    }
    Ok(())
}

pub fn checkout_local_branch(repo: &Repository, branch_name: &str) -> Result<(), git2::Error> {
    let (object, reference) = repo.revparse_ext(branch_name)?;

    // Sometimes the Cargo.lock can get out of whack, reset it before checking out
    match reset_cargo_lock(repo) {
        Ok(()) => {}
        Err(e) => {
            log::warn!("Failed to perform precautionary reset of Cargo.lock: {}", e);
        }
    };

    if !is_working_tree_clean(&repo) {
        return Err(git2::Error::from_str(
            "Workspace is not clean. Please commit or stash your changes.",
        ));
    }

    // Checkout
    repo.checkout_tree(&object, None)?;

    // Update HEAD
    match reference {
        Some(r) => {
            if r.is_branch() {
                repo.set_head(r.name().expect("Failed to set head to valid branch!"))?;
            }
        }
        None => {
            return Err(git2::Error::from_str(
                "Failed to find reference for branch!",
            ));
        }
    }

    Ok(())
}

pub fn stage_and_commit_all_changes(
    repo: &Repository,
    branch_name: &str,
    message: &str,
) -> Result<(), git2::Error> {
    log::info!(
        "⏳Staging and committing changes to branch {} with message: {}",
        branch_name,
        message
    );
    let mut index = repo.index()?;
    index.add_all(["."].iter(), IndexAddOption::DEFAULT, None)?;
    index.write()?;

    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;
    let signature = repo.signature()?;
    let parent_commit = find_last_commit_on_branch(&repo, branch_name)?;
    let new_commit_iod = repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        message,
        &tree,
        &[&parent_commit],
    )?;

    log::info!(
        "✅ Staged and committed changes to branch {}: {}",
        branch_name,
        new_commit_iod
    );

    Ok(())
}

fn find_last_commit_on_branch<'a>(
    repo: &'a Repository,
    branch_name: &'a str,
) -> Result<Commit<'a>, git2::Error> {
    let branch_ref_res = repo.find_reference(&format!("refs/heads/{}", branch_name));
    match branch_ref_res {
        Ok(branch_ref) => {
            let branch_commit_res = branch_ref.peel(ObjectType::Commit);
            match branch_commit_res {
                Ok(commit_obj) => commit_obj
                    .into_commit()
                    .map_err(|_| git2::Error::from_str("Couldn't find commit")),
                Err(e) => Err(e),
            }
        }
        Err(_) => return Err(git2::Error::from_str("branch doesn't exist!")),
    }
}

pub fn _push_to_remote(
    repo: &Repository,
    branch_name: &str,
    remote_name: &str,
) -> Result<(), git2::Error> {
    let mut remote = repo.find_remote(remote_name)?;
    let refspec = format!("refs/heads/{}:refs/heads/{}", branch_name, branch_name);
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|_url, username_from_url, _allowed_types| {
        git2::Cred::ssh_key_from_agent(
            username_from_url
                .expect("Failed to parse username from remote url. Remote must be ssh based."),
        )
    });
    let mut push_options = PushOptions::new();
    push_options.remote_callbacks(callbacks);
    remote
        .push(&[&refspec], Some(&mut push_options))
        .map_err(|e| {
            println!("Failed to push to remote: {}", e);
            e
        })?;
    Ok(())
}

/// Creates a new branch from the current HEAD and checks it out.
pub fn create_and_checkout_branch(
    repo: &Repository,
    remote_name: &str,
    branch_name: &str,
) -> Result<(), git2::Error> {
    log::info!("⏳Creating and checking out new branch {}", branch_name);
    // Get the current HEAD commit as the starting point for the new branch
    let head = repo.head()?;
    let commit = head.peel_to_commit()?;

    // Sometimes the Cargo.lock can get out of whack, reset it before checking out
    match reset_cargo_lock(repo) {
        Ok(()) => {}
        Err(e) => {
            log::warn!("Failed to perform precautionary reset of Cargo.lock: {}", e);
        }
    };

    // if the branch already exists delete it
    if let Ok(mut branch) = repo.find_branch(branch_name, git2::BranchType::Local) {
        log::warn!(
            "Branch {} already exists locally, deleting it.",
            branch_name
        );
        branch.delete()?;
    }

    // if the branch exists on the remote and delete it
    let mut remote = repo.find_remote(remote_name)?;
    let refspec = format!(":refs/heads/{}", branch_name); // : is refspec for deletion
    match remote.push(&[&refspec], Some(&mut PushOptions::new())) {
        Ok(_) => log::info!(
            "Branch {} already exists on remote, deleting it.",
            branch_name
        ),
        Err(_) => {}
    };

    // Create a new branch pointing to the current HEAD commit
    let branch = repo.branch(branch_name, &commit, false)?;

    // Get the branch's canonical name (e.g. "refs/heads/new_branch")
    let refname = branch
        .into_reference()
        .name()
        .expect("Branch name not found")
        .to_string();

    // Set the HEAD to point to the new branch
    repo.set_head(&refname)?;

    // Checkout the new branch to update the working directory
    repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;

    log::info!("✅ Created and checked out new branch {}", branch_name);

    Ok(())
}

fn reset_cargo_lock(repo: &Repository) -> Result<(), Box<dyn std::error::Error>> {
    let head = repo.head()?.peel_to_commit()?;
    let tree = head.tree()?;

    // Find the Cargo.lock blob
    let cargo_lock_entry = match tree.get_path(Path::new("Cargo.lock")) {
        Ok(entry) => entry,
        Err(e) => return Err(Box::new(e)),
    };
    let cargo_lock_blob = repo.find_blob(cargo_lock_entry.id())?;

    // Write the blob back to Cargo.toml in the working directory
    let mut cargo_lock_file = File::create(repo.path().join("Cargo.lock"))?;
    cargo_lock_file.write_all(cargo_lock_blob.content())?;

    Ok(())
}
