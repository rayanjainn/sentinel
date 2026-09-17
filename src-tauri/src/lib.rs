mod commands;
mod state;

use commands::{actions, agent, firewall, network, process, storage, system};
use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let core = state::build(app.handle())?;
            app.manage(core);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            system::get_system_info,
            system::get_resource_history,
            system::set_sampling,
            system::get_permission_status,
            system::open_permission_settings,
            process::get_process_snapshot,
            process::get_process_detail,
            process::get_process_history,
            process::reveal_process_executable,
            network::get_network_snapshot,
            network::get_geo_db_status,
            network::download_geo_db,
            network::get_home_location,
            network::set_home_location,
            firewall::get_firewall_status,
            firewall::list_firewall_rules,
            storage::list_volumes,
            storage::start_scan,
            storage::cancel_scan,
            storage::get_scan_tree,
            storage::get_scan_summary,
            storage::start_duplicate_scan,
            storage::cancel_duplicate_scan,
            storage::get_duplicate_report,
            storage::reveal_path,
            actions::prepare_action,
            actions::commit_action,
            actions::reject_action,
            actions::get_audit_log,
            agent::agent_list_providers,
            agent::agent_get_settings,
            agent::agent_update_settings,
            agent::agent_provider_status,
            agent::agent_set_api_key,
            agent::agent_delete_api_key,
            agent::agent_list_models,
            agent::agent_ollama_status,
            agent::agent_new_conversation,
            agent::agent_send_message,
            agent::agent_cancel,
            agent::agent_get_transcript,
            agent::agent_revise_plan_action,
            agent::agent_execute_plan,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start Sentinel");
}
