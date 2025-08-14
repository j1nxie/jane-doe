use semver::Version;

const ZENLESS: [&str; 16] = [
    "Business x Strangeness x Justness",
    "Cat's Lost and Found",
    "A Call From the Hollow's Heart",
    "Mission Unthinkable",
    "The Midnight Pursuit",
    "Undercover R&B",
    "Tour de Inferno",
    "Unexpected New Customer",
    "Virtual Revenge",
    "A Storm of Falling Stars",
    "Goldfinch's Escape",
    "Astra-nomical Moment",
    "Bury Your Tears With Past",
    "Wandering Visitor",
    "Where Clouds Embrace the Dawn",
    "The Impending Crash of Waves",
];

pub fn get_version() -> String {
    let semver = env!("CARGO_PKG_VERSION").parse::<Version>();
    let mut git_sha = env!("VERGEN_GIT_SHA").to_string();

    git_sha.truncate(7);

    match semver {
        Ok(semver) => {
            format!(
                "v{} - {} [[`{2}`](https://github.com/j1nxie/jane-doe/commit/{2})]",
                semver, ZENLESS[0], git_sha,
            )
        }
        Err(e) => {
            tracing::warn!(err = ?e, "couldn't parse a semver out of Cargo.toml? defaulting to 0.0.0-unknown.");
            String::from("v0.0.0-unknown - No Version Name")
        }
    }
}
