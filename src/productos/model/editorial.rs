use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Editorial {
    pub id_editorial: i32,
    pub nombre_editorial: String,
    pub rfc: Option<String>,
}

// Sirve para POST y PUT (mismos campos).
#[derive(Debug, Clone, Deserialize)]
pub struct CrearEditorialRequest {
    pub nombre_editorial: String,
    pub rfc: Option<String>,
}
