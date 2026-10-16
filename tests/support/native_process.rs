//! Reuse the production launch lock when compiling adversarial native probes.
use oxyde_lib::platform::worker_test_support::spawn;

#[path = "native_probe.rs"]
pub mod native_probe;
