//! notez desktop backend. The brains live in `notez-core`; this crate only
//! exposes them over Tauri's IPC and owns window/app lifecycle.

mod commands;
mod dto;
mod github;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(commands::BoardState::default())
        .setup(|app| {
            // Native macOS frosted-glass: the window's own background becomes
            // a blur of whatever is behind it (the desktop / other windows).
            #[cfg(target_os = "macos")]
            {
                use tauri::Manager;
                use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial, NSVisualEffectState};
                let window = app.get_webview_window("main").unwrap();
                apply_vibrancy(
                    &window,
                    NSVisualEffectMaterial::Sidebar,
                    Some(NSVisualEffectState::Active),
                    None,
                )
                .expect("failed to apply window vibrancy");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_notes,
            commands::list_notes_in_scope,
            commands::search_notes,
            commands::read_note,
            commands::get_note_tags,
            commands::set_note_tags,
            commands::list_projects,
            commands::attach_project,
            commands::detach_project,
            commands::sync,
            commands::machine_name,
            commands::migrate_preview,
            commands::migrate_apply,
            commands::create_note,
            commands::notez_root,
            commands::save_note,
            commands::append_log,
            commands::load_todo_board,
            commands::load_scope_todo_board,
            commands::toggle_task,
            commands::set_task_flags,
            commands::edit_task,
            commands::add_todo,
            commands::remove_todo,
            commands::reorder_task,
            commands::toggle_collapse,
            commands::set_task_state,
            commands::move_todo,
            commands::collapse_all,
            commands::create_category,
            github::github_user,
            github::github_all_repos,
            github::github_commits,
            github::github_issues,
            github::github_create_issue,
            github::github_contributors,
            github::github_contribution_calendar,
            github::github_repo_activity,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
