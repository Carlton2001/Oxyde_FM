//! Test-only capabilities. Production has no capability-enabled launch kind.
use super::*;

pub(super) struct Capabilities {
    arrays: Vec<(*mut PSID, u32)>,
    attributes: Vec<SID_AND_ATTRIBUTES>,
}
impl Capabilities {
    pub(super) fn new(names: &[&str]) -> io::Result<Self> {
        let mut capabilities = Self {
            arrays: Vec::new(),
            attributes: Vec::new(),
        };
        for name in names {
            let name = wide(name);
            let (mut groups, mut sids) = (std::ptr::null_mut(), std::ptr::null_mut());
            let (mut group_count, mut sid_count) = (0, 0);
            let result = unsafe {
                DeriveCapabilitySidsFromName(
                    PCWSTR(name.as_ptr()),
                    &mut groups,
                    &mut group_count,
                    &mut sids,
                    &mut sid_count,
                )
            };
            capabilities.arrays.push((groups, group_count));
            capabilities.arrays.push((sids, sid_count));
            result.map_err(native)?;
            if sids.is_null() || sid_count == 0 {
                return Err(io::Error::other("Missing capability SID"));
            }
            for sid in unsafe { std::slice::from_raw_parts(sids, sid_count as usize) } {
                capabilities.attributes.push(SID_AND_ATTRIBUTES {
                    Sid: *sid,
                    Attributes: 4,
                }); // SE_GROUP_ENABLED
            }
        }
        Ok(capabilities)
    }
    pub(super) fn apply(&self, caps: &mut SECURITY_CAPABILITIES) {
        caps.Capabilities = self.attributes.as_ptr().cast_mut();
        caps.CapabilityCount = self.attributes.len() as u32;
    }
}
impl Drop for Capabilities {
    fn drop(&mut self) {
        for (array, count) in &self.arrays {
            if !array.is_null() {
                for sid in unsafe { std::slice::from_raw_parts(*array, *count as usize) } {
                    unsafe {
                        LocalFree(Some(HLOCAL(sid.0)));
                    }
                }
                unsafe {
                    LocalFree(Some(HLOCAL((*array).cast())));
                }
            }
        }
    }
}

#[test]
fn temporary_capabilities_measure_service_compatibility_without_enabling_production_permissions() {
    use super::super::windows_process_launch::native_probe;
    for names in [
        &["lpacCom"][..],
        &["lpacCom", "registryRead"][..],
        &["lpacCom", "registryRead", "lpacMedia"][..],
    ] {
        let started = std::time::Instant::now();
        let (mut worker, output, input) = Worker::start_duplex_with_kind(
            native_probe::command("native-services"),
            512 * 1024 * 1024,
            &|| Ok(()),
            LaunchKind::LpacCapabilityProbe(names),
            None,
        )
        .unwrap();
        drop(input);
        let bytes = super::super::windows_preview_worker::collect_reply(
            output,
            started,
            std::time::Duration::from_secs(15),
            32 * 1024,
            "Native service capability probe timed out",
            &|| Ok(()),
            &mut || worker.child.try_wait(),
        )
        .unwrap();
        let report = String::from_utf8(bytes).unwrap();
        println!("LPAC temporary capabilities {names:?}: {report}");
        assert!(report.contains("com_initialize="));
    }
}
