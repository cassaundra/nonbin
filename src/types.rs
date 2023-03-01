use serde::Serialize;

#[derive(Serialize)]
pub struct UploadPaste {
    pub id: String,
    pub path: String,
    pub delete_key: String,
}
