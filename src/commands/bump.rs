use crate::common::logging::{self, Color};
use crate::common::version_extension::VersionExtension;
use crate::common::workspace::{self};

use crate::common::bump_tree::{BumpInstruction, BumpTree, ReleaseChannel};

pub fn exec_stable(
    stable_workspace: &mut workspace::Workspace,
    prerelease_workspace: &mut workspace::Workspace,
    raw_bump_instructions: Vec<&str>,
    dry_run: bool,
) -> Result<(), String> {
    log::info!("⏳Building bump tree...");
    let bump_instructions = raw_bump_instructions
        .iter()
        .map(|s| {
            BumpInstruction::from_str(
                stable_workspace,
                prerelease_workspace,
                s,
                ReleaseChannel::Stable,
            )
        })
        .collect::<Result<Vec<_>, String>>()?;

    let bump_tree = BumpTree::new(
        stable_workspace,
        prerelease_workspace,
        bump_instructions,
        ReleaseChannel::Stable,
    );

    println!("{}", bump_tree);

    if dry_run {
        log::info!("Dry-run: aborting");
        return Ok(());
    };

    let msg = format!(
        "Applying stable version bumps to branch '{}'",
        stable_workspace.branch_name
    );
    logging::bordered_message(msg.as_str(), Color::Blue);
    stable_workspace.checkout_local_branch()?;
    for (_, n) in bump_tree.highest_stable.iter() {
        let bump_instruction = n.stable.as_ref().expect("must exist here");
        let next_version = &bump_instruction
            .package
            .borrow()
            .version()
            .bump(bump_instruction.bump_type, ReleaseChannel::Stable);
        bump_instruction
            .package
            .borrow_mut()
            .set_version(&next_version);
    }

    stable_workspace.update_lockfile()?;

    stable_workspace.stage_and_commit_all(
        format!("Apply bumps {}", raw_bump_instructions.join(", ")).as_str(),
    )?;

    // TODO Actually make prerelease workspace optional
    if let Some(prerelease_workspace) = Some(&prerelease_workspace) {
        let msg = format!(
            "Applying prerelease version bumps to branch '{}'",
            prerelease_workspace.branch_name
        );
        logging::bordered_message(msg.as_str(), Color::Blue);
        prerelease_workspace.checkout_local_branch()?;

        let prerelease_branch_name = format!(
            "propagate-{}-bump-to-prerelease-{}",
            raw_bump_instructions
                .iter()
                .map(|s| s.replace(" ", "_"))
                .collect::<Vec<_>>()
                .join("-"),
            chrono::offset::Utc::now().format("%Y-%m-%d")
        );
        prerelease_workspace
            .create_and_checkout_branch(prerelease_branch_name.as_str())
            .map_err(|e| e.to_string())?;

        for (_, n) in bump_tree.highest_prerelease.iter() {
            let bump_instruction = n.prerelease.as_ref().expect("must exist here");
            let next_version = &bump_instruction
                .package
                .borrow()
                .version()
                .bump(bump_instruction.bump_type, ReleaseChannel::Stable);
            bump_instruction
                .package
                .borrow_mut()
                .set_version(&next_version);
        }

        prerelease_workspace.update_lockfile()?;
        prerelease_workspace.stage_and_commit_all(
            format!(
                "Propagate stable {} bump to prerelease",
                raw_bump_instructions.join(", ")
            )
            .as_str(),
        )?;

        log::info!("❗❗❗ Don't forget to run `git push {} {}` and open a PR to update the prerelease branch!", stable_workspace.remote_name, prerelease_branch_name);
    }

    // Check back out to the original branch before exiting.
    let msg = format!(
        "Done! Checking back out to stable branch '{}' before exiting",
        stable_workspace.branch_name
    );
    logging::bordered_message(msg.as_str(), Color::Green);
    stable_workspace.checkout_local_branch()?;

    Ok(())
}

pub fn exec_prerelease(
    stable_workspace: &mut workspace::Workspace,
    prerelease_workspace: &mut workspace::Workspace,
    raw_bump_instructions: Vec<&str>,
    dry_run: bool,
) -> Result<(), String> {
    log::info!("⏳Building bump tree...");
    let bump_instructions = raw_bump_instructions
        .iter()
        .map(|s| {
            BumpInstruction::from_str(
                stable_workspace,
                prerelease_workspace,
                s,
                ReleaseChannel::Prerelease,
            )
        })
        .collect::<Result<Vec<_>, String>>()?;

    let bump_tree = BumpTree::new(
        stable_workspace,
        prerelease_workspace,
        bump_instructions,
        ReleaseChannel::Prerelease,
    );
    prerelease_workspace.checkout_local_branch()?;

    if bump_tree.root_nodes.is_empty() {
        logging::bordered_message("No bumps to apply, exiting early.", Color::Green);
        return Ok(());
    }

    println!("{}", bump_tree);

    if dry_run {
        log::info!("Dry-run: aborting");
        return Ok(());
    }

    let msg = format!(
        "Applying prerelease version bumps to branch '{}'",
        prerelease_workspace.branch_name
    );
    logging::bordered_message(msg.as_str(), Color::Blue);
    for (_, n) in bump_tree.highest_prerelease.iter() {
        let bump_instruction = n.prerelease.as_ref().expect("must exist here");
        let next_version = &bump_instruction
            .package
            .borrow()
            .version()
            .bump(bump_instruction.bump_type, ReleaseChannel::Prerelease);
        bump_instruction
            .package
            .borrow_mut()
            .set_version(&next_version);
    }

    prerelease_workspace.update_lockfile()?;

    prerelease_workspace.stage_and_commit_all(
        format!("Apply bumps {}", raw_bump_instructions.join(", ")).as_str(),
    )?;

    Ok(())
}
