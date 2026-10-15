//! End-to-end checks of the storage layer against the `DESIGN.md` "Test
//! scenarios": the real `notez` binary and the real `git`, with Alice's
//! laptop (home `<tmp>/alice`) and desktop (home `<tmp>/alice-desk`) as two
//! fake machines sharing bare remotes in the same temp dir.

mod common;

use common::{describe, find_named, find_symlinks, remote_files, World};

/// Scenario A: a personal note written on the laptop lands in the vault's
/// `personal/<project>/`, syncs through the notez remote to the desktop
/// (different home, different clone path), and never enters the project.
#[test]
fn scenario_a_personal_note_follows_alice_between_machines() {
    let world = World::new();
    let project_remote = world.project_remote("shared-repo-1");
    let vault_remote = world.bare_remote("alice-notez");

    let laptop = world.machine("alice");
    laptop.init_vault(&vault_remote);
    let laptop_project = laptop.clone_project(&project_remote, "repos/shared-repo-1");
    laptop.notez_ok(&laptop_project, &["attach"]);

    let desktop = world.machine("alice-desk");
    desktop.clone_vault(&vault_remote);
    let desktop_project = desktop.clone_project(&project_remote, "Repos/shared-repo-1");
    desktop.notez_ok(&desktop_project, &["attach"]);

    let note = laptop.create_note(
        &laptop_project,
        &["-p", "add", "API redesign idea", "Split the v2 endpoints."],
    );

    let laptop_dir = laptop.vault().join("personal").join("shared-repo-1");
    assert_eq!(
        note.parent(),
        Some(laptop_dir.as_path()),
        "note at {note:?}"
    );
    let file_name = note.file_name().unwrap().to_str().unwrap().to_string();
    assert!(
        file_name.ends_with("api-redesign-idea.md"),
        "file name {file_name}"
    );
    let content = std::fs::read_to_string(&note).unwrap();
    assert!(
        content.contains("Split the v2 endpoints."),
        "content:\n{content}"
    );

    laptop.notez_ok(&laptop_project, &["sync"]);
    let tracked = laptop.git_ok(&laptop.vault(), &["ls-files", "personal/shared-repo-1"]);
    assert!(
        tracked
            .lines()
            .any(|l| l == format!("personal/shared-repo-1/{file_name}")),
        "not tracked in the vault:\n{tracked}"
    );

    desktop.notez_ok(&desktop_project, &["sync"]);
    let on_desktop = desktop
        .vault()
        .join("personal")
        .join("shared-repo-1")
        .join(&file_name);
    assert_eq!(std::fs::read_to_string(&on_desktop).unwrap(), content);

    for (machine, project) in [(&laptop, &laptop_project), (&desktop, &desktop_project)] {
        assert!(
            find_named(project, &file_name).is_empty(),
            "note inside {project:?}"
        );
        let status = machine.git_ok(project, &["status", "--porcelain", "--ignored"]);
        assert!(
            status.is_empty(),
            "project tree touched in {project:?}:\n{status}"
        );
    }
    let project_files = remote_files(&laptop, &project_remote);
    assert!(
        !project_files.iter().any(|f| f.ends_with(&file_name)),
        "note reached the project remote: {project_files:?}"
    );
}

/// Scenario C: a scratch note lands in `<repo>/.notez/`, notez gitignores
/// that directory, and the note reaches neither the notez remote nor the
/// project remote even after a habitual `git add -A` and push.
#[test]
fn scenario_c_scratch_note_stays_on_the_laptop() {
    let world = World::new();
    let project_remote = world.project_remote("shared-repo-1");
    let vault_remote = world.bare_remote("alice-notez");

    let laptop = world.machine("alice");
    laptop.init_vault(&vault_remote);
    let project = laptop.clone_project(&project_remote, "repos/shared-repo-1");

    let note = laptop.create_note(
        &project,
        &[
            "-l",
            "add",
            "try this branch out",
            "Rebase onto main first.",
        ],
    );
    assert_eq!(
        note.parent(),
        Some(project.join(".notez").as_path()),
        "note at {note:?}"
    );
    let file_name = note.file_name().unwrap().to_str().unwrap().to_string();

    let gitignore = std::fs::read_to_string(project.join(".gitignore")).unwrap();
    assert!(
        gitignore.lines().any(|l| l.trim() == ".notez"),
        ".gitignore:\n{gitignore}"
    );
    let rel = format!(".notez/{file_name}");
    let ignored = laptop.git(&project, &["check-ignore", "-q", &rel]);
    assert!(
        ignored.status.success(),
        "{rel} is not ignored:\n{}",
        describe(&ignored)
    );

    laptop.notez_ok(&project, &["sync"]);
    laptop.git_ok(&project, &["add", "-A"]);
    laptop.git_ok(&project, &["commit", "-q", "-m", "work"]);
    laptop.git_ok(&project, &["push", "-q"]);

    let vault_files = remote_files(&laptop, &vault_remote);
    assert!(
        !vault_files.iter().any(|f| f.ends_with(&file_name)),
        "scratch note reached the notez remote: {vault_files:?}"
    );
    let project_files = remote_files(&laptop, &project_remote);
    assert!(
        !project_files
            .iter()
            .any(|f| f.starts_with(".notez") || f.ends_with(&file_name)),
        "scratch note reached the project remote: {project_files:?}"
    );

    let desktop = world.machine("alice-desk");
    desktop.clone_vault(&vault_remote);
    let desktop_project = desktop.clone_project(&project_remote, "Repos/shared-repo-1");
    desktop.notez_ok(&desktop_project, &["sync"]);
    desktop.git_ok(&desktop_project, &["pull", "-q"]);
    assert!(
        !desktop_project.join(".notez").exists(),
        "scratch store reached the desktop"
    );
    assert!(
        find_named(&desktop.home, &file_name).is_empty(),
        "scratch note on the desktop"
    );
}

/// Scenario E: the same project lives at `~/repos/...` on the laptop and
/// `~/Repos/...` on the desktop. Each registry is per machine, stores its own
/// tilde-relative path, and `notez list` shows each machine its own path.
/// The laptop attaches explicitly; the desktop relies on auto-registration
/// of a repository that already has a `notez/` folder.
#[test]
fn scenario_e_each_machine_resolves_its_own_project_path() {
    let world = World::new();
    let project_remote = world.project_remote("shared-repo-1");
    let vault_remote = world.bare_remote("alice-notez");

    let laptop = world.machine("alice");
    laptop.init_vault(&vault_remote);
    let laptop_project = laptop.clone_project(&project_remote, "repos/shared-repo-1");
    let attached = laptop.notez_ok(&laptop_project, &["attach"]);
    assert!(
        attached.contains("Attached shared-repo-1 at ~/repos/shared-repo-1"),
        "attach said:\n{attached}"
    );

    let desktop = world.machine("alice-desk");
    desktop.clone_vault(&vault_remote);
    let desktop_project = desktop.clone_project(&project_remote, "Repos/shared-repo-1");
    let first = desktop.notez(&desktop_project, &["list"]);
    assert!(first.status.success(), "{}", describe(&first));
    let stderr = String::from_utf8_lossy(&first.stderr);
    assert!(
        stderr.contains("registered shared-repo-1 (~/Repos/shared-repo-1)"),
        "no auto-registration:\n{}",
        describe(&first)
    );

    let laptop_registry = std::fs::read_to_string(laptop.registry_file()).unwrap();
    let desktop_registry = std::fs::read_to_string(desktop.registry_file()).unwrap();
    assert!(
        laptop_registry.contains(r#"local_path = "~/repos/shared-repo-1""#),
        "{laptop_registry}"
    );
    assert!(
        desktop_registry.contains(r#"local_path = "~/Repos/shared-repo-1""#),
        "{desktop_registry}"
    );

    for (machine, shown) in [
        (&laptop, "~/repos/shared-repo-1"),
        (&desktop, "~/Repos/shared-repo-1"),
    ] {
        let listed = machine.notez_ok(&machine.home, &["list"]);
        let line = listed
            .lines()
            .find(|l| l.contains("shared-repo-1"))
            .unwrap_or_else(|| panic!("project missing from list:\n{listed}"));
        assert!(line.contains(shown), "expected {shown} in {line:?}");
        assert!(!line.contains("not found"), "project unreachable: {line:?}");
    }

    let laptop_note = laptop.create_note(
        &laptop_project,
        &["-p", "add", "laptop idea", "From the laptop."],
    );
    let desktop_note = desktop.create_note(
        &desktop_project,
        &["-p", "add", "desktop idea", "From the desktop."],
    );
    assert_eq!(
        laptop_note.parent(),
        Some(laptop.vault().join("personal/shared-repo-1").as_path())
    );
    assert_eq!(
        desktop_note.parent(),
        Some(desktop.vault().join("personal/shared-repo-1").as_path())
    );

    laptop.notez_ok(&laptop.home, &["sync"]);
    desktop.notez_ok(&desktop.home, &["sync"]);
    let vault_files = remote_files(&laptop, &vault_remote);
    assert!(
        !vault_files.iter().any(|f| f.ends_with("registry.toml")),
        "a registry was synced: {vault_files:?}"
    );
    for machine in [&laptop, &desktop] {
        let links = find_symlinks(&machine.home);
        assert!(
            links.is_empty(),
            "symlinks under {:?}: {links:?}",
            machine.home
        );
        assert!(find_symlinks(&machine.xdg).is_empty());
    }
}

/// Scenario I: the project moves on the laptop. `notez list` marks the stale
/// entry instead of crashing, `notez attach --path` repairs it, and the
/// desktop's registry is unaffected.
#[test]
fn scenario_i_moved_project_is_flagged_then_reattached() {
    let world = World::new();
    let project_remote = world.project_remote("shared-repo-1");

    let laptop = world.machine("alice");
    let old_path = laptop.clone_project(&project_remote, "repos/shared-repo-1");
    laptop.notez_ok(&old_path, &["attach"]);

    let desktop = world.machine("alice-desk");
    let desktop_project = desktop.clone_project(&project_remote, "Repos/shared-repo-1");
    desktop.notez_ok(&desktop_project, &["attach"]);

    let new_path = laptop.home.join("Code/work/shared-repo-1");
    std::fs::create_dir_all(new_path.parent().unwrap()).unwrap();
    std::fs::rename(&old_path, &new_path).unwrap();

    let stale = laptop.notez(&laptop.home, &["list"]);
    assert!(
        stale.status.success(),
        "list crashed on a missing project:\n{}",
        describe(&stale)
    );
    let stdout = String::from_utf8_lossy(&stale.stdout);
    let line = stdout
        .lines()
        .find(|l| l.contains("shared-repo-1"))
        .unwrap_or_else(|| panic!("project missing from list:\n{stdout}"));
    assert!(line.contains("~/repos/shared-repo-1"), "line {line:?}");
    assert!(
        line.contains("(not found)"),
        "missing project not flagged: {line:?}"
    );

    let reattached = laptop.notez_ok(
        &laptop.home,
        &[
            "attach",
            "shared-repo-1",
            "--path",
            "~/Code/work/shared-repo-1",
        ],
    );
    assert!(
        reattached.contains("Attached shared-repo-1 at ~/Code/work/shared-repo-1 (updated)"),
        "attach said:\n{reattached}"
    );

    let fixed = laptop.notez_ok(&laptop.home, &["list"]);
    let line = fixed
        .lines()
        .find(|l| l.contains("shared-repo-1"))
        .unwrap_or_else(|| panic!("project missing from list:\n{fixed}"));
    assert!(line.contains("~/Code/work/shared-repo-1"), "line {line:?}");
    assert!(!line.contains("not found"), "still flagged: {line:?}");

    let note = laptop.create_note(
        &new_path,
        &["-p", "add", "after the move", "Still resolves."],
    );
    assert_eq!(
        note.parent(),
        Some(laptop.vault().join("personal/shared-repo-1").as_path())
    );

    let desktop_list = desktop.notez_ok(&desktop.home, &["list"]);
    let line = desktop_list
        .lines()
        .find(|l| l.contains("shared-repo-1"))
        .unwrap_or_else(|| panic!("project missing from desktop list:\n{desktop_list}"));
    assert!(
        line.contains("~/Repos/shared-repo-1"),
        "desktop line {line:?}"
    );
    assert!(!line.contains("not found"), "desktop affected: {line:?}");
}

/// Scenario J: both machines edit the same vault file without syncing in
/// between. The first sync wins, the second stops with the manual-resolution
/// hint and loses nothing, and after a hand resolution both machines agree.
#[test]
fn scenario_j_conflicting_edits_stop_sync_without_losing_either_side() {
    let world = World::new();
    let vault_remote = world.bare_remote("alice-notez");

    let laptop = world.machine("alice");
    laptop.init_vault(&vault_remote);
    let desktop = world.machine("alice-desk");
    desktop.clone_vault(&vault_remote);

    let note = laptop.create_note(
        &laptop.home,
        &["-g", "add", "plan", "Shared starting point."],
    );
    let rel = note
        .strip_prefix(laptop.vault())
        .unwrap_or_else(|_| panic!("global note outside the vault: {note:?}"))
        .to_str()
        .unwrap()
        .to_string();
    laptop.notez_ok(&laptop.home, &["sync"]);
    desktop.notez_ok(&desktop.home, &["sync"]);
    let desktop_note = desktop.vault().join(&rel);
    assert!(
        desktop_note.is_file(),
        "seed note did not reach the desktop"
    );

    let laptop_text = "# plan\n\nLaptop says ship on Friday.\n";
    let desktop_text = "# plan\n\nDesktop says ship on Monday.\n";
    std::fs::write(&note, laptop_text).unwrap();
    std::fs::write(&desktop_note, desktop_text).unwrap();

    laptop.notez_ok(&laptop.home, &["sync"]);

    let second = desktop.notez(&desktop.home, &["sync"]);
    assert!(
        !second.status.success(),
        "conflicting sync succeeded:\n{}",
        describe(&second)
    );
    let stderr = String::from_utf8_lossy(&second.stderr);
    assert!(
        stderr.contains("resolve conflicts manually and rerun"),
        "no resolution hint:\n{}",
        describe(&second)
    );

    let in_tree = std::fs::read_to_string(&desktop_note).unwrap();
    assert!(
        in_tree.contains("Desktop says ship on Monday."),
        "desktop edit lost:\n{in_tree}"
    );
    let committed = desktop.git_ok(&desktop.vault(), &["show", &format!("ORIG_HEAD:{rel}")]);
    assert_eq!(committed, desktop_text, "desktop commit lost");
    let on_remote = laptop.git_ok(&vault_remote, &["show", &format!("main:{rel}")]);
    assert_eq!(on_remote, laptop_text, "remote lost the laptop edit");

    let resolved = "# plan\n\nLaptop says ship on Friday.\nDesktop says ship on Monday.\n";
    std::fs::write(&desktop_note, resolved).unwrap();
    desktop.git_ok(&desktop.vault(), &["add", &rel]);
    desktop.git_ok(&desktop.vault(), &["rebase", "--continue"]);

    desktop.notez_ok(&desktop.home, &["sync"]);
    laptop.notez_ok(&laptop.home, &["sync"]);

    assert_eq!(std::fs::read_to_string(&note).unwrap(), resolved);
    assert_eq!(std::fs::read_to_string(&desktop_note).unwrap(), resolved);
    assert_eq!(
        laptop.git_ok(&vault_remote, &["show", &format!("main:{rel}")]),
        resolved
    );
}
