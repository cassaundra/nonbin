use serde::Serialize;

#[derive(Serialize)]
pub struct UploadPaste {
    pub id: String,
    pub path: String,
}
