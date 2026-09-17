//! Bundled catalog of remote endpoints, matched on the reverse-DNS name or a well-known address.
//!
//! A match names who owns the other end and what that endpoint is generally used for. It never
//! claims to know what was sent: for encrypted ports the caller adds an explicit sentence saying
//! the contents cannot be read.

use std::net::IpAddr;

pub(super) struct Endpoint {
    /// Who runs the other end: "Apple", "Cloudflare".
    pub owner: &'static str,
    /// What this endpoint is usually for.
    pub purpose: &'static str,
    /// True for endpoints whose job is collecting usage, crash or advertising data.
    pub telemetry: bool,
}

const fn owner(owner: &'static str, purpose: &'static str) -> Endpoint {
    Endpoint {
        owner,
        purpose,
        telemetry: false,
    }
}

const fn telemetry(owner: &'static str, purpose: &'static str) -> Endpoint {
    Endpoint {
        owner,
        purpose,
        telemetry: true,
    }
}

/// Longest suffix wins, so `googlevideo.com` beats `google.com`.
const SUFFIXES: &[(&str, Endpoint)] = &[
    // Apple.
    (
        "push.apple.com",
        owner(
            "Apple",
            "the push-notification service that tells Messages, Mail and apps when something arrives",
        ),
    ),
    (
        "icloud.com",
        owner(
            "Apple",
            "iCloud: syncing files, photos, notes and app data for your Apple ID",
        ),
    ),
    (
        "icloud-content.com",
        owner("Apple", "downloading and uploading iCloud file contents"),
    ),
    (
        "swcdn.apple.com",
        owner("Apple", "downloading macOS updates"),
    ),
    (
        "mzstatic.com",
        owner("Apple", "App Store and media artwork"),
    ),
    (
        "aaplimg.com",
        owner("Apple", "Apple's own content delivery network"),
    ),
    (
        "cdn-apple.com",
        owner(
            "Apple",
            "Apple's content delivery network for app and media downloads",
        ),
    ),
    ("ls.apple.com", owner("Apple", "Maps location services")),
    (
        "apple.com",
        owner(
            "Apple",
            "Apple services: activation, updates, the App Store and iCloud",
        ),
    ),
    // Google.
    (
        "googlevideo.com",
        owner("Google", "YouTube video streaming"),
    ),
    (
        "doubleclick.net",
        telemetry("Google", "advertising and tracking"),
    ),
    (
        "google-analytics.com",
        telemetry("Google", "website usage statistics"),
    ),
    (
        "analytics.google.com",
        telemetry("Google", "website usage statistics"),
    ),
    (
        "googletagmanager.com",
        telemetry(
            "Google",
            "loading analytics and advertising tags for websites",
        ),
    ),
    (
        "crashlytics.com",
        telemetry("Google", "app crash reporting"),
    ),
    (
        "firebaseinstallations.googleapis.com",
        telemetry("Google", "identifying an app install for Firebase services"),
    ),
    (
        "gstatic.com",
        owner(
            "Google",
            "static files — fonts, scripts and images — that Google sites load",
        ),
    ),
    (
        "googleusercontent.com",
        owner(
            "Google",
            "user-uploaded content such as profile pictures and Drive files",
        ),
    ),
    (
        "googleapis.com",
        owner(
            "Google",
            "Google's APIs, used by both Google apps and third-party apps",
        ),
    ),
    (
        "1e100.net",
        owner(
            "Google",
            "Google's own network; the name is how Google labels its addresses",
        ),
    ),
    ("dns.google", owner("Google", "Google's public DNS service")),
    ("google.com", owner("Google", "Google search and services")),
    ("gmail.com", owner("Google", "Gmail")),
    // Microsoft.
    ("windowsupdate.com", owner("Microsoft", "Windows updates")),
    (
        "msedge.net",
        owner("Microsoft", "Microsoft's content delivery network"),
    ),
    (
        "azureedge.net",
        owner(
            "Microsoft",
            "Azure's content delivery network, used by many apps and sites",
        ),
    ),
    (
        "office.com",
        owner("Microsoft", "Microsoft 365 documents and mail"),
    ),
    (
        "live.com",
        owner("Microsoft", "Microsoft account sign-in and Outlook"),
    ),
    (
        "microsoft.com",
        owner("Microsoft", "Microsoft services, sign-in and updates"),
    ),
    (
        "msftconnecttest.com",
        owner(
            "Microsoft",
            "checking whether this machine really has internet access",
        ),
    ),
    (
        "github.com",
        owner(
            "GitHub (Microsoft)",
            "code hosting: repositories, pull requests and releases",
        ),
    ),
    (
        "githubusercontent.com",
        owner(
            "GitHub (Microsoft)",
            "raw files and release downloads from GitHub",
        ),
    ),
    // Infrastructure and CDNs.
    (
        "cloudflare-dns.com",
        owner("Cloudflare", "Cloudflare's public DNS service"),
    ),
    (
        "cloudflare.net",
        owner(
            "Cloudflare",
            "Cloudflare's content delivery network, which fronts a large share of websites",
        ),
    ),
    (
        "cloudflare.com",
        owner(
            "Cloudflare",
            "Cloudflare's network, which fronts a large share of websites",
        ),
    ),
    (
        "workers.dev",
        owner("Cloudflare", "an app or API hosted on Cloudflare Workers"),
    ),
    (
        "cloudfront.net",
        owner(
            "Amazon Web Services",
            "CloudFront: the delivery network many sites and apps use",
        ),
    ),
    (
        "amazonaws.com",
        owner(
            "Amazon Web Services",
            "servers and services hosted on AWS, used by a large share of apps",
        ),
    ),
    (
        "akamaiedge.net",
        owner(
            "Akamai",
            "a content delivery network that fronts many large sites",
        ),
    ),
    (
        "akamaitechnologies.com",
        owner(
            "Akamai",
            "a content delivery network that fronts many large sites",
        ),
    ),
    (
        "akadns.net",
        owner("Akamai", "Akamai's traffic-steering DNS"),
    ),
    (
        "akamaized.net",
        owner("Akamai", "content delivered through Akamai"),
    ),
    (
        "fastly.net",
        owner(
            "Fastly",
            "a content delivery network that fronts many sites and app APIs",
        ),
    ),
    (
        "fastlylb.net",
        owner("Fastly", "Fastly's load balancing network"),
    ),
    (
        "edgekey.net",
        owner("Akamai", "content delivered through Akamai"),
    ),
    (
        "edgesuite.net",
        owner("Akamai", "content delivered through Akamai"),
    ),
    // Consumer services.
    (
        "fbcdn.net",
        owner(
            "Meta",
            "images and video for Facebook, Instagram and WhatsApp",
        ),
    ),
    ("facebook.com", owner("Meta", "Facebook")),
    ("instagram.com", owner("Meta", "Instagram")),
    ("whatsapp.net", owner("Meta", "WhatsApp messaging")),
    ("nflxvideo.net", owner("Netflix", "Netflix video streaming")),
    ("netflix.com", owner("Netflix", "Netflix")),
    ("scdn.co", owner("Spotify", "Spotify audio streaming")),
    ("spotify.com", owner("Spotify", "Spotify")),
    ("slack.com", owner("Slack (Salesforce)", "Slack messaging")),
    ("zoom.us", owner("Zoom", "Zoom meetings")),
    (
        "discord.com",
        owner("Discord", "Discord messaging and calls"),
    ),
    (
        "discordapp.net",
        owner("Discord", "Discord media and voice"),
    ),
    ("telegram.org", owner("Telegram", "Telegram messaging")),
    ("dropbox.com", owner("Dropbox", "Dropbox file syncing")),
    (
        "twitch.tv",
        owner("Twitch (Amazon)", "Twitch live streaming"),
    ),
    // Developer services.
    (
        "registry.npmjs.org",
        owner("npm (GitHub)", "downloading JavaScript packages"),
    ),
    (
        "npmjs.org",
        owner("npm (GitHub)", "JavaScript package registry"),
    ),
    (
        "pythonhosted.org",
        owner("Python Package Index", "downloading Python packages"),
    ),
    (
        "pypi.org",
        owner("Python Package Index", "Python package registry"),
    ),
    ("crates.io", owner("crates.io", "Rust package registry")),
    ("docker.io", owner("Docker", "downloading container images")),
    (
        "docker.com",
        owner("Docker", "Docker services and image downloads"),
    ),
    (
        "jetbrains.com",
        owner("JetBrains", "IDE licensing and updates"),
    ),
    (
        "anthropic.com",
        owner("Anthropic", "the Claude API or claude.ai"),
    ),
    ("openai.com", owner("OpenAI", "the OpenAI API or ChatGPT")),
    ("ollama.com", owner("Ollama", "downloading local AI models")),
    ("ollama.ai", owner("Ollama", "downloading local AI models")),
    (
        "huggingface.co",
        owner("Hugging Face", "downloading models and datasets"),
    ),
    // Analytics and error reporting, labelled as such.
    (
        "sentry.io",
        telemetry("Sentry", "collecting error and crash reports from an app"),
    ),
    (
        "segment.io",
        telemetry("Segment (Twilio)", "collecting product usage events"),
    ),
    (
        "segment.com",
        telemetry("Segment (Twilio)", "collecting product usage events"),
    ),
    (
        "amplitude.com",
        telemetry("Amplitude", "collecting product usage statistics"),
    ),
    (
        "mixpanel.com",
        telemetry("Mixpanel", "collecting product usage statistics"),
    ),
    (
        "posthog.com",
        telemetry("PostHog", "collecting product usage statistics"),
    ),
    (
        "datadoghq.com",
        telemetry("Datadog", "sending application metrics and logs"),
    ),
    (
        "bugsnag.com",
        telemetry("Bugsnag", "collecting crash reports"),
    ),
    (
        "plausible.io",
        telemetry("Plausible", "website visit statistics"),
    ),
    (
        "matomo.cloud",
        telemetry("Matomo", "website visit statistics"),
    ),
    (
        "newrelic.com",
        telemetry("New Relic", "sending application performance data"),
    ),
    (
        "sentry-cdn.com",
        telemetry("Sentry", "loading the error-reporting script"),
    ),
];

/// Addresses whose owner is a matter of public record.
const ADDRESSES: &[(&str, Endpoint)] = &[
    ("8.8.8.8", owner("Google", "Google's public DNS service")),
    ("8.8.4.4", owner("Google", "Google's public DNS service")),
    (
        "1.1.1.1",
        owner("Cloudflare", "Cloudflare's public DNS service"),
    ),
    (
        "1.0.0.1",
        owner("Cloudflare", "Cloudflare's public DNS service"),
    ),
    (
        "9.9.9.9",
        owner(
            "Quad9",
            "Quad9's public DNS service, which filters known-malicious names",
        ),
    ),
    (
        "208.67.222.222",
        owner("OpenDNS (Cisco)", "OpenDNS's public DNS service"),
    ),
    (
        "208.67.220.220",
        owner("OpenDNS (Cisco)", "OpenDNS's public DNS service"),
    ),
    (
        "2001:4860:4860::8888",
        owner("Google", "Google's public DNS service"),
    ),
    (
        "2001:4860:4860::8844",
        owner("Google", "Google's public DNS service"),
    ),
    (
        "2606:4700:4700::1111",
        owner("Cloudflare", "Cloudflare's public DNS service"),
    ),
];

pub(super) fn by_hostname(host: &str) -> Option<&'static Endpoint> {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    SUFFIXES
        .iter()
        .filter(|(suffix, _)| host == *suffix || host.ends_with(&format!(".{suffix}")))
        .max_by_key(|(suffix, _)| suffix.len())
        .map(|(_, endpoint)| endpoint)
}

pub(super) fn by_address(addr: IpAddr) -> Option<&'static Endpoint> {
    if let Some(endpoint) = ADDRESSES
        .iter()
        .find(|(text, _)| text.parse::<IpAddr>().is_ok_and(|known| known == addr))
        .map(|(_, endpoint)| endpoint)
    {
        return Some(endpoint);
    }
    // Apple owns all of 17.0.0.0/8.
    if let IpAddr::V4(v4) = addr
        && v4.octets()[0] == 17
    {
        return Some(&APPLE_RANGE);
    }
    None
}

static APPLE_RANGE: Endpoint = Endpoint {
    owner: "Apple",
    purpose: "an Apple service: the whole 17.x.x.x address range belongs to Apple",
    telemetry: false,
};
