use super::windows_com::{take_string, ComApartment, PropertyValue};
use windows::core::{GUID, PCWSTR};
use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::System::Com::IBindCtx;
use windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc;
use windows::Win32::UI::Shell::PropertiesSystem::{
    IPropertyStore, SHGetPropertyStoreFromParsingName, GPS_DEFAULT,
};

pub(super) fn friendly_name(path: &str) -> Result<Option<String>, String> {
    let _com = ComApartment::mta_or_existing().map_err(|error| error.to_string())?;
    let wide: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
    unsafe {
        let store: IPropertyStore = SHGetPropertyStoreFromParsingName::<_, Option<&IBindCtx>, _>(
            PCWSTR(wide.as_ptr()),
            None,
            GPS_DEFAULT,
        )
        .map_err(|error| error.to_string())?;
        let key = PROPERTYKEY {
            fmtid: GUID::from_values(
                0xA45C2502,
                0xDFC5,
                0x491D,
                [0x8F, 0x1D, 0xDC, 0x14, 0x11, 0x51, 0xD4, 0xB2],
            ),
            pid: 14,
        };
        let value = PropertyValue::new(store.GetValue(&key).map_err(|error| error.to_string())?);
        let name = take_string(
            PropVariantToStringAlloc(value.as_ref()).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        Ok((!name.is_empty()).then_some(name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_property_value_and_converted_names_have_separate_ownership() {
        std::thread::spawn(|| {
            let directory = tempfile::tempdir().unwrap();
            let name = "Données 日本語 🦀.oxyde_fixture";
            let path = directory.path().join(name);
            std::fs::write(&path, b"fixture").unwrap();
            let _com = ComApartment::mta_or_existing().unwrap();
            let wide: Vec<u16> = path
                .to_str()
                .unwrap()
                .encode_utf16()
                .chain(Some(0))
                .collect();
            unsafe {
                let store: IPropertyStore =
                    SHGetPropertyStoreFromParsingName::<_, Option<&IBindCtx>, _>(
                        PCWSTR(wide.as_ptr()),
                        None,
                        GPS_DEFAULT,
                    )
                    .unwrap();
                // PKEY_ItemNameDisplay, as defined by the Windows SDK.
                let key = PROPERTYKEY {
                    fmtid: GUID::from_u128(0xb725f130_47ef_101a_a5f1_02608c9eebac),
                    pid: 10,
                };
                let value = PropertyValue::new(store.GetValue(&key).unwrap());
                // Free each converted string before reading the same owned value again.
                for _ in 0..2 {
                    let converted = PropVariantToStringAlloc(value.as_ref()).unwrap();
                    assert_eq!(take_string(converted).unwrap(), name);
                }
            }
        })
        .join()
        .unwrap();
    }
}
