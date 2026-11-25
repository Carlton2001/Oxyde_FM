use super::*;

#[test]
fn reads_the_new_label_or_association_name_only_for_registered_new_types() {
    let manifest = r#"
        <Package xmlns:files="http://schemas.microsoft.com/appx/manifest/uap/windows10"
                 xmlns:create="http://schemas.microsoft.com/appx/manifest/uap/windows10/4"
                 xmlns:app="http://schemas.microsoft.com/appx/manifest/uap/windows10/10">
          <files:FileTypeAssociation Name="test">
            <files:DisplayName>Default &amp; document</files:DisplayName>
            <files:SupportedFileTypes>
              <files:FileType create:ShellNewFileName="empty.bmp"
                 create:ShellNewDisplayName="ms-resource:Resources/NewBitmap">.bmp</files:FileType>
              <files:FileType app:ShellNewCommandParameters="%1">.script</files:FileType>
              <files:FileType>.read</files:FileType>
              <files:FileType create:ShellNewFileName="empty">not-an-extension</files:FileType>
            </files:SupportedFileTypes>
          </files:FileTypeAssociation>
        </Package>"#;
    let mut types = Vec::new();
    visit_manifest(manifest, |label, extension, program| {
        types.push((label.to_owned(), extension.to_owned(), program))
    });
    assert_eq!(
        types,
        [
            ("ms-resource:Resources/NewBitmap".into(), ".bmp".into(), false),
            ("Default & document".into(), ".script".into(), true),
        ]
    );
}
