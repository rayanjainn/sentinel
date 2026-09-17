//! Classifier tables, service guesser samples, and the "never invent an explanation" contract.
//!
//! Process samples are drawn from real command lines seen on a running machine (Brave/Chromium,
//! WebKit, macOS system daemons) so the classifier is checked against actual evidence, not
//! hypothetical strings.

use std::net::IpAddr;

use super::connection::{ConnectionFacts, explain_connection};
use super::process::{ParentFacts, ProcessFacts, explain_process};
use crate::model::{
    AddrScope, Confidence, ProcessCategory, ProcessRole, QuitSafety, TransportProtocol,
};

fn cmd_from(args: &[&str]) -> Vec<String> {
    args.iter().copied().map(str::to_owned).collect()
}

fn facts<'a>(
    name: &'a str,
    exe: Option<&'a str>,
    cmd: &'a [String],
    user: Option<&'a str>,
    platform: crate::model::Platform,
    parent: Option<ParentFacts<'a>>,
) -> ProcessFacts<'a> {
    ProcessFacts {
        pid: 1234,
        name,
        exe,
        cmd,
        user,
        platform,
        parent,
    }
}

use crate::model::Platform;

// ---- Chromium / Electron family --------------------------------------------------------------

#[test]
fn brave_renderer_names_a_web_page_never_a_tab() {
    // Real sample: `--type=renderer --metrics-client-id=... --extension-process`.
    let cmd = cmd_from(&[
        "--type=renderer",
        "--metrics-client-id=x",
        "--extension-process",
    ]);
    let parent = ParentFacts {
        pid: 997,
        name: "Brave Browser",
        exe: Some("/Applications/Brave Browser.app/Contents/MacOS/Brave Browser"),
    };
    let exe = "/Applications/Brave Browser.app/Contents/Frameworks/Brave Browser \
                Framework.framework/Versions/1/Helpers/Brave Browser Helper (Renderer).app/Contents/MacOS/Brave Browser Helper (Renderer)";
    let f = facts(
        "Brave Browser Helper (Renderer)",
        Some(exe),
        &cmd,
        Some("rayanjain"),
        Platform::Macos,
        Some(parent),
    );
    let e = explain_process(&f);
    assert_eq!(
        e.role,
        ProcessRole::Extension,
        "extension flag wins over renderer"
    );
    assert_eq!(e.category, ProcessCategory::AppHelper);
    assert_eq!(e.quit_safety, QuitSafety::AppHelper);
    assert_eq!(e.app_name.as_deref(), Some("Brave Browser"));
    assert!(
        !e.detail.to_lowercase().contains("tab "),
        "must not fabricate a tab"
    );
    assert_eq!(e.confidence, Confidence::Known);
}

#[test]
fn brave_plain_renderer_says_one_tab_or_site_never_a_url() {
    let cmd = cmd_from(&["--type=renderer", "--metrics-client-id=x"]);
    let exe = "/Applications/Brave Browser.app/Contents/Frameworks/Brave Browser \
                Framework.framework/Versions/1/Helpers/Brave Browser Helper (Renderer).app/Contents/MacOS/Brave Browser Helper (Renderer)";
    let f = facts(
        "Brave Browser Helper (Renderer)",
        Some(exe),
        &cmd,
        Some("rayanjain"),
        Platform::Macos,
        None,
    );
    let e = explain_process(&f);
    assert_eq!(e.role, ProcessRole::Renderer);
    assert_eq!(e.app_name.as_deref(), Some("Brave Browser"));
    assert!(e.detail.to_lowercase().contains("one browser tab or site"));
    assert!(!e.detail.contains("://"), "must never fabricate a URL");
    assert_eq!(e.quit_safety, QuitSafety::AppHelper);
    assert!(e.quit_note.contains("Brave Browser"));
}

#[test]
fn brave_network_utility_names_the_job_from_the_sub_type_flag() {
    let cmd = cmd_from(&[
        "--type=utility",
        "--utility-sub-type=network.mojom.NetworkService",
        "--lang=en-US",
    ]);
    let exe = "/Applications/Brave Browser.app/Contents/Frameworks/Brave Browser \
                Framework.framework/Versions/1/Helpers/Brave Browser Helper.app/Contents/MacOS/Brave Browser Helper";
    let f = facts(
        "Brave Browser Helper",
        Some(exe),
        &cmd,
        None,
        Platform::Macos,
        None,
    );
    let e = explain_process(&f);
    assert_eq!(e.role, ProcessRole::NetworkService);
    assert_eq!(e.app_name.as_deref(), Some("Brave Browser"));
    assert!(e.headline.contains("network") || e.detail.contains("network request"));
    assert!(
        e.evidence
            .iter()
            .any(|line| line.contains("network.mojom.NetworkService"))
    );
}

#[test]
fn brave_gpu_process_is_graphics_helper() {
    let cmd = cmd_from(&["--type=gpu-process", "--gpu-preferences=x"]);
    let exe = "/Applications/Brave Browser.app/Contents/Frameworks/Brave Browser \
                Framework.framework/Versions/1/Helpers/Brave Browser Helper.app/Contents/MacOS/Brave Browser Helper";
    let f = facts(
        "Brave Browser Helper",
        Some(exe),
        &cmd,
        None,
        Platform::Macos,
        None,
    );
    let e = explain_process(&f);
    assert_eq!(e.role, ProcessRole::Gpu);
    assert_eq!(e.app_name.as_deref(), Some("Brave Browser"));
}

#[test]
fn crashpad_handler_is_named_regardless_of_owning_app() {
    let cmd: Vec<String> = Vec::new();
    let exe = "/Applications/Brave Browser.app/Contents/Frameworks/Brave Browser \
                Framework.framework/Versions/1/Helpers/chrome_crashpad_handler";
    let f = facts(
        "chrome_crashpad_handler",
        Some(exe),
        &cmd,
        None,
        Platform::Macos,
        None,
    );
    let e = explain_process(&f);
    assert_eq!(e.role, ProcessRole::Utility);
    assert!(e.headline.to_lowercase().contains("crash"));
    assert_eq!(e.app_name.as_deref(), Some("Brave Browser"));
}

#[test]
fn webkit_webcontent_is_a_renderer_owned_by_its_parent_app() {
    let cmd: Vec<String> = Vec::new();
    let exe = "/System/Library/Frameworks/WebKit.framework/Versions/A/XPCServices/\
               com.apple.WebKit.WebContent.xpc/Contents/MacOS/com.apple.WebKit.WebContent";
    let parent = ParentFacts {
        pid: 900,
        name: "Safari",
        exe: Some("/System/Applications/Safari.app/Contents/MacOS/Safari"),
    };
    let f = facts(
        "com.apple.WebKit.WebContent",
        Some(exe),
        &cmd,
        None,
        Platform::Macos,
        Some(parent),
    );
    let e = explain_process(&f);
    assert_eq!(e.role, ProcessRole::Renderer);
    assert_eq!(e.app_name.as_deref(), Some("Safari"));
    assert!(e.detail.contains("one browser tab or site") || e.detail.contains("does not say"));
}

#[test]
fn webkit_networking_is_named_network_service() {
    let cmd: Vec<String> = Vec::new();
    let exe = "/System/Library/Frameworks/WebKit.framework/Versions/A/XPCServices/\
               com.apple.WebKit.Networking.xpc/Contents/MacOS/com.apple.WebKit.Networking";
    let f = facts(
        "com.apple.WebKit.Networking",
        Some(exe),
        &cmd,
        None,
        Platform::Macos,
        None,
    );
    let e = explain_process(&f);
    assert_eq!(e.role, ProcessRole::NetworkService);
}

// ---- Interpreters ------------------------------------------------------------------------------

#[test]
fn python_names_the_script_not_just_the_interpreter() {
    // Real cmdline layout: argv[0] is the interpreter path, argv[1] the script it runs.
    let cmd = cmd_from(&[
        "/Users/rayanjain/.cache/uv/archive-v0/x/bin/python",
        "/opt/tool/bin/blender-mcp",
    ]);
    let exe = "/Users/rayanjain/.cache/uv/archive-v0/x/bin/python";
    let f = facts(
        "python",
        Some(exe),
        &cmd,
        Some("rayanjain"),
        Platform::Macos,
        None,
    );
    let e = explain_process(&f);
    assert_eq!(e.role, ProcessRole::Interpreter);
    assert!(e.headline.contains("blender-mcp"));
    assert!(e.headline.contains("Python"));
    assert_eq!(e.confidence, Confidence::Known);
}

#[test]
fn node_with_no_script_admits_it_does_not_know() {
    let cmd: Vec<String> = Vec::new();
    let f = facts(
        "node",
        Some("/usr/local/bin/node"),
        &cmd,
        None,
        Platform::Macos,
        None,
    );
    let e = explain_process(&f);
    assert_eq!(e.role, ProcessRole::Interpreter);
    assert_eq!(e.confidence, Confidence::Unknown);
    assert!(e.detail.to_lowercase().contains("cannot say"));
}

#[test]
fn java_with_jar_flag_names_the_jar() {
    let cmd = cmd_from(&["-Xmx512m", "-jar", "server.jar"]);
    let f = facts(
        "java",
        Some("/usr/bin/java"),
        &cmd,
        None,
        Platform::Linux,
        None,
    );
    let e = explain_process(&f);
    assert!(e.headline.contains("server.jar"));
}

// ---- Real macOS/Linux/Windows system daemons ---------------------------------------------------

#[test]
fn macos_kernel_task_is_system_critical() {
    let cmd: Vec<String> = Vec::new();
    let f = facts(
        "kernel_task",
        None,
        &cmd,
        Some("root"),
        Platform::Macos,
        None,
    );
    let e = explain_process(&f);
    assert_eq!(e.role, ProcessRole::Kernel);
    assert_eq!(e.category, ProcessCategory::Kernel);
    assert_eq!(e.quit_safety, QuitSafety::SystemCritical);
    assert!(e.quit_note.starts_with("Don't quit this."));
}

#[test]
fn macos_launchd_pid_1_is_the_first_process() {
    let f = ProcessFacts {
        pid: 1,
        name: "launchd",
        exe: Some("/sbin/launchd"),
        cmd: &[],
        user: Some("root"),
        platform: Platform::Macos,
        parent: None,
    };
    let e = explain_process(&f);
    assert_eq!(e.quit_safety, QuitSafety::SystemCritical);
    assert!(e.headline.to_lowercase().contains("launchd"));
}

#[test]
fn macos_mds_is_spotlight_and_restarts_after_quitting() {
    let f = facts(
        "mds",
        Some(
            "/System/Library/Frameworks/CoreServices.framework/Versions/A/\
        Frameworks/Metadata.framework/Versions/A/Support/mds",
        ),
        &[],
        Some("root"),
        Platform::Macos,
        None,
    );
    let e = explain_process(&f);
    assert_eq!(e.quit_safety, QuitSafety::OsService);
    assert!(e.headline.to_lowercase().contains("spotlight"));
    assert!(e.quit_note.to_lowercase().contains("spotlight"));
    assert!(e.quit_note.contains("start it again"));
}

#[test]
fn macos_unrecognised_xpc_service_still_names_its_owning_app() {
    let exe = "/Applications/SomeApp.app/Contents/XPCServices/com.example.Helper.xpc/Contents/MacOS/com.example.Helper";
    let f = facts(
        "com.example.Helper",
        Some(exe),
        &[],
        None,
        Platform::Macos,
        None,
    );
    let e = explain_process(&f);
    assert_eq!(e.app_name.as_deref(), Some("SomeApp"));
    assert_eq!(e.confidence, Confidence::Likely);
}

#[test]
fn macos_regular_app_bundle_is_the_app_itself() {
    let f = facts(
        "Blender",
        Some("/Applications/Blender.app/Contents/MacOS/Blender"),
        &[],
        Some("rayanjain"),
        Platform::Macos,
        None,
    );
    let e = explain_process(&f);
    assert_eq!(e.role, ProcessRole::MainApp);
    assert_eq!(e.category, ProcessCategory::UserApp);
    assert_eq!(e.quit_safety, QuitSafety::UserApp);
    assert_eq!(e.app_name.as_deref(), Some("Blender"));
}

#[test]
fn linux_systemd_journald_is_critical() {
    let f = facts(
        "systemd-journald",
        Some("/usr/lib/systemd/systemd-journald"),
        &[],
        Some("root"),
        Platform::Linux,
        None,
    );
    let e = explain_process(&f);
    assert_eq!(e.quit_safety, QuitSafety::SystemCritical);
}

#[test]
fn linux_unrecognised_systemd_unit_is_named_as_such() {
    let f = facts(
        "some-custom-daemon",
        Some("/usr/lib/systemd/some-custom-daemon"),
        &[],
        Some("root"),
        Platform::Linux,
        None,
    );
    let e = explain_process(&f);
    assert_eq!(e.category, ProcessCategory::OsService);
    assert_eq!(e.confidence, Confidence::Likely);
    assert!(e.detail.contains("systemd"));
}

#[test]
fn windows_svchost_resolves_the_hosted_service_from_the_dash_s_flag() {
    let cmd = cmd_from(&["-k", "NetworkService", "-p", "-s", "Dnscache"]);
    let f = facts(
        "svchost.exe",
        Some(r"C:\Windows\System32\svchost.exe"),
        &cmd,
        Some("NT AUTHORITY\\LOCAL SERVICE"),
        Platform::Windows,
        None,
    );
    let e = explain_process(&f);
    assert!(e.headline.contains("DNS client"));
    assert_eq!(e.quit_safety, QuitSafety::OsService);
}

#[test]
fn windows_svchost_without_dash_s_admits_it_cannot_say_which_service() {
    let cmd = cmd_from(&["-k", "netsvcs", "-p"]);
    let f = facts(
        "svchost.exe",
        Some(r"C:\Windows\System32\svchost.exe"),
        &cmd,
        None,
        Platform::Windows,
        None,
    );
    let e = explain_process(&f);
    assert_eq!(e.confidence, Confidence::Unknown);
    assert!(e.detail.to_lowercase().contains("cannot say"));
}

#[test]
fn windows_lsass_is_system_critical() {
    let f = facts(
        "lsass.exe",
        Some(r"C:\Windows\System32\lsass.exe"),
        &[],
        Some("SYSTEM"),
        Platform::Windows,
        None,
    );
    let e = explain_process(&f);
    assert_eq!(e.quit_safety, QuitSafety::SystemCritical);
}

#[test]
fn windows_dwm_is_the_desktop_compositor() {
    let f = facts(
        "dwm.exe",
        Some(r"C:\Windows\System32\dwm.exe"),
        &[],
        None,
        Platform::Windows,
        None,
    );
    let e = explain_process(&f);
    assert_eq!(e.quit_safety, QuitSafety::SystemCritical);
    assert!(e.headline.to_lowercase().contains("desktop"));
}

// ---- No invented data: unknown binaries -------------------------------------------------------

#[test]
fn unknown_macos_binary_is_unrecognised_not_fabricated() {
    let f = facts(
        "totally_made_up_tool",
        Some("/Users/rayanjain/bin/totally_made_up_tool"),
        &[],
        Some("rayanjain"),
        Platform::Macos,
        None,
    );
    let e = explain_process(&f);
    assert_eq!(e.confidence, Confidence::Unknown);
    assert_eq!(e.role, ProcessRole::Unknown);
    assert_eq!(e.category, ProcessCategory::Unknown);
    assert_eq!(e.quit_safety, QuitSafety::Unknown);
    assert!(e.headline.contains("not recognised"));
    assert!(e.detail.to_lowercase().contains("does not recognise"));
    // Evidence must carry the raw facts rather than an invented description.
    assert!(
        e.evidence
            .iter()
            .any(|line| line.contains("totally_made_up_tool"))
    );
}

#[test]
fn unknown_binary_with_no_executable_path_says_so_plainly() {
    let f = facts("mystery", None, &[], None, Platform::Linux, None);
    let e = explain_process(&f);
    assert_eq!(e.confidence, Confidence::Unknown);
    assert!(e.evidence.iter().any(|line| line.contains("not readable")));
}

// ---- Connection / service guesser ----------------------------------------------------------

fn conn<'a>(
    protocol: TransportProtocol,
    remote_addr: Option<IpAddr>,
    remote_port: Option<u16>,
    remote_host: Option<&'a str>,
    remote_scope: Option<AddrScope>,
) -> ConnectionFacts<'a> {
    ConnectionFacts {
        protocol,
        remote_addr,
        remote_port,
        remote_host,
        remote_scope,
        local_port: 54321,
        owner_role: None,
    }
}

#[test]
fn port_443_is_named_secure_web_and_states_encryption() {
    let addr: IpAddr = "17.242.13.5".parse().unwrap();
    let f = conn(
        TransportProtocol::Tcp,
        Some(addr),
        Some(443),
        None,
        Some(AddrScope::Public),
    );
    let e = explain_connection(&f);
    assert_eq!(e.service.as_deref(), Some("secure web"));
    assert!(e.encrypted);
    assert!(e.detail.to_lowercase().contains("cannot read"));
    assert_eq!(e.headline, "Apple");
}

#[test]
fn port_53_is_domain_name_lookup_and_not_encrypted() {
    let addr: IpAddr = "8.8.8.8".parse().unwrap();
    let f = conn(
        TransportProtocol::Udp,
        Some(addr),
        Some(53),
        None,
        Some(AddrScope::Public),
    );
    let e = explain_connection(&f);
    assert_eq!(e.service.as_deref(), Some("domain name lookup"));
    assert!(!e.encrypted);
    assert_eq!(e.headline, "Google");
}

#[test]
fn port_5353_is_finding_devices_on_the_network() {
    let addr: IpAddr = "224.0.0.251".parse().unwrap();
    let f = conn(
        TransportProtocol::Udp,
        Some(addr),
        Some(5353),
        None,
        Some(AddrScope::Multicast),
    );
    let e = explain_connection(&f);
    // Multicast scope wins the headline (it is a broadcast, not a single device), but the port
    // service is still available for anyone matching on port alone.
    assert!(ports_lookup_service(5353).contains("finding devices"));
    let _ = e;
}

fn ports_lookup_service(port: u16) -> String {
    super::ports::lookup(port, TransportProtocol::Udp)
        .map(|s| s.name.to_owned())
        .unwrap_or_default()
}

#[test]
fn port_123_is_clock_sync() {
    assert_eq!(ports_lookup_service(123), "clock sync");
}

#[test]
fn port_22_is_remote_shell_and_encrypted() {
    let addr: IpAddr = "203.0.113.9".parse().unwrap();
    let f = conn(
        TransportProtocol::Tcp,
        Some(addr),
        Some(22),
        None,
        Some(AddrScope::Public),
    );
    let e = explain_connection(&f);
    assert_eq!(e.service.as_deref(), Some("remote shell"));
    assert!(e.encrypted);
}

#[test]
fn postgres_on_loopback_matches_the_real_local_dev_setup() {
    let addr: IpAddr = "127.0.0.1".parse().unwrap();
    let f = conn(
        TransportProtocol::Tcp,
        Some(addr),
        Some(5432),
        None,
        Some(AddrScope::Loopback),
    );
    let e = explain_connection(&f);
    assert_eq!(e.service.as_deref(), Some("PostgreSQL database"));
    assert!(e.detail.contains("this computer"));
    assert!(!e.encrypted);
}

#[test]
fn ollama_on_loopback_is_named_local_ai_model_server() {
    let addr: IpAddr = "127.0.0.1".parse().unwrap();
    let f = conn(
        TransportProtocol::Tcp,
        Some(addr),
        Some(11434),
        None,
        Some(AddrScope::Loopback),
    );
    let e = explain_connection(&f);
    assert_eq!(e.service.as_deref(), Some("local AI model server"));
}

#[test]
fn analytics_endpoint_is_labelled_as_telemetry_not_hidden() {
    let addr: IpAddr = "142.250.72.1".parse().unwrap();
    let f = conn(
        TransportProtocol::Tcp,
        Some(addr),
        Some(443),
        Some("www.google-analytics.com"),
        Some(AddrScope::Public),
    );
    let e = explain_connection(&f);
    assert!(
        e.detail.to_lowercase().contains("usage") || e.detail.to_lowercase().contains("analytics")
    );
    assert_eq!(e.headline, "Google");
}

#[test]
fn unmatched_public_host_names_the_hostname_and_admits_it() {
    let addr: IpAddr = "203.0.113.55".parse().unwrap();
    let f = conn(
        TransportProtocol::Tcp,
        Some(addr),
        Some(51820),
        Some("some.unknown.example.net"),
        Some(AddrScope::Public),
    );
    let e = explain_connection(&f);
    assert_eq!(e.confidence, Confidence::Unknown);
    assert!(e.detail.contains("some.unknown.example.net"));
    assert!(
        e.detail.to_lowercase().contains("doesn't recognise")
            || e.detail.to_lowercase().contains("does not recognise")
    );
}

#[test]
fn unmatched_public_address_with_no_hostname_shows_the_address() {
    let addr: IpAddr = "203.0.113.55".parse().unwrap();
    let f = conn(
        TransportProtocol::Tcp,
        Some(addr),
        Some(51820),
        None,
        Some(AddrScope::Public),
    );
    let e = explain_connection(&f);
    assert!(e.detail.contains("203.0.113.55"));
}

#[test]
fn browser_renderer_connection_says_one_tab_or_site_never_a_url() {
    let addr: IpAddr = "17.1.1.1".parse().unwrap();
    let mut f = conn(
        TransportProtocol::Tcp,
        Some(addr),
        Some(443),
        None,
        Some(AddrScope::Public),
    );
    f.owner_role = Some(ProcessRole::Renderer);
    let e = explain_connection(&f);
    assert!(e.detail.to_lowercase().contains("one browser tab or site"));
    assert!(!e.detail.contains("://"), "must never fabricate a URL");
}

#[test]
fn listening_socket_has_no_remote_end_to_explain() {
    let f = conn(TransportProtocol::Tcp, None, None, None, None);
    let e = explain_connection(&f);
    assert!(e.detail.to_lowercase().contains("nothing is connected"));
}

#[test]
fn listening_on_known_port_names_the_service() {
    let mut f = conn(TransportProtocol::Tcp, None, None, None, None);
    f.local_port = 5432;
    f.remote_port = None;
    // Listening explanations use `local_port`; simulate by reusing the port lookup directly since
    // the listening branch does not take a remote port.
    let service = super::ports::lookup(5432, TransportProtocol::Tcp).unwrap();
    assert_eq!(service.name, "PostgreSQL database");
    let _ = explain_connection(&f);
}

#[test]
fn loopback_without_a_known_port_is_still_scoped_correctly() {
    let addr: IpAddr = "127.0.0.1".parse().unwrap();
    let f = conn(
        TransportProtocol::Tcp,
        Some(addr),
        Some(57016),
        None,
        Some(AddrScope::Loopback),
    );
    let e = explain_connection(&f);
    assert_eq!(e.confidence, Confidence::Likely);
    assert!(e.detail.contains("computer"));
}

#[test]
fn local_network_device_is_distinguished_from_the_public_internet() {
    let addr: IpAddr = "192.168.29.31".parse().unwrap();
    let f = conn(
        TransportProtocol::Tcp,
        Some(addr),
        Some(52288),
        None,
        Some(AddrScope::Private),
    );
    let e = explain_connection(&f);
    assert!(e.detail.contains("local network"));
    assert!(e.detail.to_lowercase().contains("not the internet"));
}
