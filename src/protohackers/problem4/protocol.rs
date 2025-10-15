pub enum Message {
    Insert { key: String, value: String },
    Retrieve { key: String },
    VersionReport,
}

//
