mod commands;
mod git;
mod process;
mod terminal;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(commands::NetworkOperationState::default())
        .manage(terminal::TerminalManager::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::detect_git,
            commands::load_repository,
            commands::change_files,
            commands::load_diff,
            commands::list_history,
            commands::list_unpushed_commits,
            commands::history_fingerprint,
            commands::load_history_detail,
            commands::load_history_diff,
            commands::load_commit_context,
            commands::list_remotes,
            commands::list_remote_details,
            commands::list_branches,
            commands::create_branch,
            commands::switch_branch,
            commands::rename_branch,
            commands::delete_branch,
            commands::create_commit,
            commands::repository_operation_state,
            commands::advanced_preview,
            commands::run_advanced_operation,
            commands::run_operation_action,
            commands::push,
            commands::fetch,
            commands::pull,
            commands::cancel_network_operation,
            commands::terminal_create,
            commands::terminal_write,
            commands::terminal_write_bytes,
            commands::terminal_resize,
            commands::terminal_close,
            commands::terminal_close_all,
            commands::terminal_list,
            commands::terminal_history,
            commands::terminal_clear_history
        ])
        .build(tauri::generate_context!())
        .unwrap_or_else(|error| {
            eprintln!("Failed to start Git GUI: {error}");
            std::process::exit(1);
        })
        .run(|app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                let terminal = app_handle.state::<terminal::TerminalManager>();
                let _ = terminal.close_all();
            }
        });
}
