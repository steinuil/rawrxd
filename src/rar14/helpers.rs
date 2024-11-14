// DOS filenames only included ASCII characters, excluding some special characters.
// https://superuser.com/questions/1362080/which-characters-are-invalid-for-an-ms-dos-filename
pub fn conv_dos_filename(name: Vec<u8>) -> String {
    name.into_iter()
        .map(|c| match c as char {
            '\\' => '/',
            c => c,
        })
        .collect()
}
