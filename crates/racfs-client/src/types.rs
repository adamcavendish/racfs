//! Types for RACFS client

use serde::{Deserialize, Serialize};

/// Health check response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

/// Capabilities response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilitiesResponse {
    pub features: Vec<String>,
    #[serde(rename = "max_file_size")]
    pub max_file_size: u64,
}

/// File metadata from server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadataResponse {
    #[serde(rename = "file_type")]
    pub file_type: String,
    pub permissions: String,
    pub size: u64,
    pub modified: Option<String>,
    pub path: String,
    /// If this is a symlink, the target path.
    pub symlink_target: Option<String>,
}

/// Directory entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryEntry {
    pub permissions: String,
    pub size: u64,
    pub modified: Option<String>,
    pub path: String,
    /// File type: "file", "directory", "symlink", etc.
    #[serde(rename = "file_type")]
    pub file_type: Option<String>,
}

/// Directory list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryListResponse {
    pub entries: Vec<DirectoryEntry>,
}

/// Generic message response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageResponse {
    pub message: String,
}

/// File query parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileQuery {
    pub path: String,
}

/// Write file request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteRequest {
    pub path: String,
    pub data: String,
    pub offset: Option<i64>,
}

/// Create directory request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDirectoryRequest {
    pub path: String,
    pub perm: Option<u32>,
}

/// Rename request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameRequest {
    #[serde(rename = "old_path")]
    pub old_path: String,
    #[serde(rename = "new_path")]
    pub new_path: String,
}

/// Chmod request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChmodRequest {
    pub path: String,
    pub mode: u32,
}

/// Truncate request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TruncateRequest {
    pub path: String,
    pub size: u64,
}

/// Symlink request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymlinkRequest {
    pub target: String,
    #[serde(rename = "link_path")]
    pub link_path: String,
}

/// Xattr get/list query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XattrQuery {
    pub path: String,
    pub name: String,
}

/// Xattr list query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XattrListQuery {
    pub path: String,
}

/// Set xattr request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetXattrRequest {
    pub path: String,
    pub name: String,
    pub value: String, // base64
}

/// Xattr list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XattrListResponse {
    pub names: Vec<String>,
}

/// Xattr value response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XattrValueResponse {
    pub value: String, // base64
}

// === Authentication types ===

/// Login request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// Login response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub refresh_token: Option<String>,
}

/// User info response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserResponse {
    pub id: String,
    pub username: String,
    pub role: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_health_response_serialization() {
        let response = HealthResponse {
            status: "ok".to_string(),
            version: "1.0.0".to_string(),
        };
        let json = serde_json::to_string(&response).expect("Failed to serialize");
        assert!(json.contains("\"status\":\"ok\""));
        assert!(json.contains("\"version\":\"1.0.0\""));
    }

    #[test]
    fn test_health_response_deserialization() {
        let json = r#"{"status": "ok", "version": "1.0.0"}"#;
        let response: HealthResponse = serde_json::from_str(json).expect("Failed to deserialize");
        assert_eq!(response.status, "ok");
        assert_eq!(response.version, "1.0.0");
    }

    #[test]
    fn test_capabilities_response_serialization() {
        let response = CapabilitiesResponse {
            features: vec!["read".to_string(), "write".to_string()],
            max_file_size: 1024 * 1024,
        };
        let json = serde_json::to_string(&response).expect("Failed to serialize");
        assert!(json.contains("\"max_file_size\":1048576"));
    }

    #[test]
    fn test_capabilities_response_deserialization() {
        let json = r#"{"features": ["read", "write"], "max_file_size": 1048576}"#;
        let response: CapabilitiesResponse =
            serde_json::from_str(json).expect("Failed to deserialize");
        assert_eq!(response.features, vec!["read", "write"]);
        assert_eq!(response.max_file_size, 1048576);
    }

    #[test]
    fn test_file_metadata_response_serialization() {
        let response = FileMetadataResponse {
            file_type: "file".to_string(),
            permissions: "rw-r--r--".to_string(),
            size: 1024,
            modified: Some("2024-01-01T00:00:00Z".to_string()),
            path: "/test/file.txt".to_string(),
            symlink_target: None,
        };
        let json = serde_json::to_string(&response).expect("Failed to serialize");
        assert!(json.contains("\"file_type\":\"file\""));
    }

    #[test]
    fn test_file_metadata_response_deserialization() {
        let json = r#"{
            "file_type": "file",
            "permissions": "rw-r--r--",
            "size": 1024,
            "modified": "2024-01-01T00:00:00Z",
            "path": "/test/file.txt"
        }"#;
        let response: FileMetadataResponse =
            serde_json::from_str(json).expect("Failed to deserialize");
        assert_eq!(response.file_type, "file");
        assert_eq!(response.size, 1024);
        assert!(response.modified.is_some());
    }

    #[test]
    fn test_directory_entry_serialization() {
        let entry = DirectoryEntry {
            permissions: "rwxr-xr-x".to_string(),
            size: 4096,
            modified: Some("2024-01-01T00:00:00Z".to_string()),
            path: "/test/dir".to_string(),
            file_type: None,
        };
        let json = serde_json::to_string(&entry).expect("Failed to serialize");
        assert!(json.contains("\"path\":\"/test/dir\""));
    }

    #[test]
    fn test_directory_entry_deserialization() {
        let json = r#"{
            "permissions": "rwxr-xr-x",
            "size": 4096,
            "modified": "2024-01-01T00:00:00Z",
            "path": "/test/dir"
        }"#;
        let entry: DirectoryEntry = serde_json::from_str(json).expect("Failed to deserialize");
        assert_eq!(entry.permissions, "rwxr-xr-x");
        assert_eq!(entry.size, 4096);
    }

    #[test]
    fn test_directory_list_response() {
        let response = DirectoryListResponse {
            entries: vec![
                DirectoryEntry {
                    permissions: "rw-r--r--".to_string(),
                    size: 100,
                    modified: None,
                    path: "/test/file1.txt".to_string(),
                    file_type: None,
                },
                DirectoryEntry {
                    permissions: "rwxr-xr-x".to_string(),
                    size: 200,
                    modified: None,
                    path: "/test/file2.txt".to_string(),
                    file_type: None,
                },
            ],
        };
        let json = serde_json::to_string(&response).expect("Failed to serialize");
        assert!(json.contains("entries"));

        let decoded: DirectoryListResponse =
            serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(decoded.entries.len(), 2);
    }

    #[test]
    fn test_message_response() {
        let response = MessageResponse {
            message: "File created successfully".to_string(),
        };
        let json = serde_json::to_string(&response).expect("Failed to serialize");
        assert!(json.contains("\"message\":\"File created successfully\""));

        let decoded: MessageResponse = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(decoded.message, "File created successfully");
    }

    #[test]
    fn test_file_query_serialization() {
        let query = FileQuery {
            path: "/test/file.txt".to_string(),
        };
        let json = serde_json::to_string(&query).expect("Failed to serialize");
        assert!(json.contains("\"path\":\"/test/file.txt\""));
    }

    #[test]
    fn test_write_request_serialization() {
        let request = WriteRequest {
            path: "/test/file.txt".to_string(),
            data: "Hello, World!".to_string(),
            offset: Some(0),
        };
        let json = serde_json::to_string(&request).expect("Failed to serialize");
        assert!(json.contains("\"path\":\"/test/file.txt\""));
        assert!(json.contains("\"offset\":0"));

        let decoded: WriteRequest = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(decoded.offset, Some(0));
    }

    #[test]
    fn test_write_request_without_offset() {
        let request = WriteRequest {
            path: "/test/file.txt".to_string(),
            data: "Hello, World!".to_string(),
            offset: None,
        };
        let json = serde_json::to_string(&request).expect("Failed to serialize");
        assert!(json.contains("\"offset\":null"));
    }

    #[test]
    fn test_create_directory_request_serialization() {
        let request = CreateDirectoryRequest {
            path: "/test/dir".to_string(),
            perm: Some(0o755),
        };
        let json = serde_json::to_string(&request).expect("Failed to serialize");
        assert!(json.contains("\"path\":\"/test/dir\""));
        assert!(json.contains("\"perm\":493")); // 0o755 = 493

        let decoded: CreateDirectoryRequest =
            serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(decoded.perm, Some(0o755));
    }

    #[test]
    fn test_rename_request_serialization() {
        let request = RenameRequest {
            old_path: "/test/old.txt".to_string(),
            new_path: "/test/new.txt".to_string(),
        };
        let json = serde_json::to_string(&request).expect("Failed to serialize");
        assert!(json.contains("\"old_path\":\"/test/old.txt\""));
        assert!(json.contains("\"new_path\":\"/test/new.txt\""));

        let decoded: RenameRequest = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(decoded.old_path, "/test/old.txt");
        assert_eq!(decoded.new_path, "/test/new.txt");
    }

    #[test]
    fn test_chmod_request_serialization() {
        let request = ChmodRequest {
            path: "/test/file.txt".to_string(),
            mode: 0o644,
        };
        let json = serde_json::to_string(&request).expect("Failed to serialize");
        assert!(json.contains("\"path\":\"/test/file.txt\""));
        assert!(json.contains("\"mode\":420")); // 0o644 = 420

        let decoded: ChmodRequest = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(decoded.mode, 0o644);
    }

    #[test]
    fn test_truncate_request_roundtrip() {
        let request = TruncateRequest {
            path: "/test/file.txt".to_string(),
            size: 1024,
        };
        let json = serde_json::to_string(&request).expect("Failed to serialize");
        let decoded: TruncateRequest = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(decoded.path, request.path);
        assert_eq!(decoded.size, request.size);
    }

    #[test]
    fn test_symlink_request_roundtrip() {
        let request = SymlinkRequest {
            target: "/target".to_string(),
            link_path: "/link".to_string(),
        };
        let json = serde_json::to_string(&request).expect("Failed to serialize");
        assert!(json.contains("\"link_path\":\"/link\""));
        let decoded: SymlinkRequest = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(decoded.target, "/target");
        assert_eq!(decoded.link_path, "/link");
    }

    #[test]
    fn test_login_request_roundtrip() {
        let request = LoginRequest {
            username: "alice".to_string(),
            password: "secret".to_string(),
        };
        let json = serde_json::to_string(&request).expect("Failed to serialize");
        let decoded: LoginRequest = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(decoded.username, "alice");
        assert_eq!(decoded.password, "secret");
    }

    #[test]
    fn test_login_response_roundtrip() {
        let response = LoginResponse {
            access_token: "jwt.here".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: 3600,
            refresh_token: Some("refresh.here".to_string()),
        };
        let json = serde_json::to_string(&response).expect("Failed to serialize");
        let decoded: LoginResponse = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(decoded.access_token, "jwt.here");
        assert_eq!(decoded.token_type, "Bearer");
        assert_eq!(decoded.expires_in, 3600);
        assert_eq!(decoded.refresh_token.as_deref(), Some("refresh.here"));
    }

    #[test]
    fn test_user_response_roundtrip() {
        let response = UserResponse {
            id: "user-1".to_string(),
            username: "bob".to_string(),
            role: "admin".to_string(),
        };
        let json = serde_json::to_string(&response).expect("Failed to serialize");
        let decoded: UserResponse = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(decoded.id, "user-1");
        assert_eq!(decoded.username, "bob");
        assert_eq!(decoded.role, "admin");
    }

    #[test]
    fn test_xattr_query_roundtrip() {
        let query = XattrQuery {
            path: "/file".to_string(),
            name: "user.key".to_string(),
        };
        let json = serde_json::to_string(&query).expect("Failed to serialize");
        let decoded: XattrQuery = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(decoded.path, "/file");
        assert_eq!(decoded.name, "user.key");
    }

    #[test]
    fn test_set_xattr_request_roundtrip() {
        let request = SetXattrRequest {
            path: "/file".to_string(),
            name: "user.foo".to_string(),
            value: "YmFy".to_string(), // base64 "bar"
        };
        let json = serde_json::to_string(&request).expect("Failed to serialize");
        let decoded: SetXattrRequest = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(decoded.value, "YmFy");
    }

    #[test]
    fn test_xattr_list_query_roundtrip() {
        let query = XattrListQuery {
            path: "/dir".to_string(),
        };
        let json = serde_json::to_string(&query).expect("Failed to serialize");
        let decoded: XattrListQuery = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(decoded.path, "/dir");
    }

    #[test]
    fn test_xattr_list_response_roundtrip() {
        let response = XattrListResponse {
            names: vec!["user.a".to_string(), "user.b".to_string()],
        };
        let json = serde_json::to_string(&response).expect("Failed to serialize");
        let decoded: XattrListResponse =
            serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(decoded.names, vec!["user.a", "user.b"]);
    }

    #[test]
    fn test_xattr_value_response_roundtrip() {
        let response = XattrValueResponse {
            value: "dmFs".to_string(), // base64
        };
        let json = serde_json::to_string(&response).expect("Failed to serialize");
        let decoded: XattrValueResponse =
            serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(decoded.value, "dmFs");
    }
}

#[cfg(test)]
mod proptest_tests {
    use super::*;
    use proptest::prelude::*;

    fn path_string() -> impl Strategy<Value = String> {
        prop::string::string_regex("[a-zA-Z0-9_./-]*")
            .expect("valid regex")
            .prop_map(|s| {
                if s.is_empty() {
                    "/".to_string()
                } else {
                    "/".to_string() + &s
                }
            })
    }

    proptest! {
        #[test]
        fn write_request_roundtrip(path in path_string(), data in ".*", offset in prop::option::of(any::<i64>())) {
            let req = WriteRequest { path: path.clone(), data: data.clone(), offset };
            let json = serde_json::to_string(&req).unwrap();
            let decoded: WriteRequest = serde_json::from_str(&json).unwrap();
            assert_eq!(decoded.path, req.path);
            assert_eq!(decoded.data, req.data);
            assert_eq!(decoded.offset, req.offset);
        }

        #[test]
        fn rename_request_roundtrip(old_path in path_string(), new_path in path_string()) {
            let req = RenameRequest { old_path: old_path.clone(), new_path: new_path.clone() };
            let json = serde_json::to_string(&req).unwrap();
            let decoded: RenameRequest = serde_json::from_str(&json).unwrap();
            assert_eq!(decoded.old_path, req.old_path);
            assert_eq!(decoded.new_path, req.new_path);
        }

        #[test]
        fn chmod_request_roundtrip(path in path_string(), mode in 0u32..=0o7777u32) {
            let req = ChmodRequest { path: path.clone(), mode };
            let json = serde_json::to_string(&req).unwrap();
            let decoded: ChmodRequest = serde_json::from_str(&json).unwrap();
            assert_eq!(decoded.path, req.path);
            assert_eq!(decoded.mode, req.mode);
        }

        #[test]
        fn truncate_request_roundtrip(path in path_string(), size in 0u64..=u64::MAX) {
            let req = TruncateRequest { path: path.clone(), size };
            let json = serde_json::to_string(&req).unwrap();
            let decoded: TruncateRequest = serde_json::from_str(&json).unwrap();
            assert_eq!(decoded.path, req.path);
            assert_eq!(decoded.size, req.size);
        }
    }
}
