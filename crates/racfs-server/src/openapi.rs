use utoipa::OpenApi;

use crate::api::directories::{
    CreateDirectoryRequest, DirectoryEntry, DirectoryListResponse, DirectoryQuery,
    DirectoryResponse,
};
use crate::api::files::{FileQuery, FileResponse, WriteRequest};
use crate::api::metadata::{
    ActionResponse, ChmodRequest, MetadataResponse, RenameRequest, StatQuery, SymlinkRequest,
    TruncateRequest,
};
use crate::api::xattr::{
    SetXattrRequest, XattrListQuery, XattrListResponse, XattrQuery, XattrValueResponse,
};

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::api::files::read_file,
        crate::api::files::create_file,
        crate::api::files::write_file,
        crate::api::files::delete_file,
        crate::api::directories::list_directory,
        crate::api::directories::create_directory,
        crate::api::metadata::stat,
        crate::api::metadata::rename,
        crate::api::metadata::chmod,
        crate::api::metadata::truncate,
        crate::api::metadata::symlink,
        crate::api::xattr::get_xattr,
        crate::api::xattr::set_xattr,
        crate::api::xattr::remove_xattr,
        crate::api::xattr::list_xattr,
        crate::api::health::health_check,
    ),
    components(
        schemas(
            FileQuery,
            WriteRequest,
            FileResponse,
            DirectoryQuery,
            CreateDirectoryRequest,
            DirectoryResponse,
            DirectoryListResponse,
            DirectoryEntry,
            StatQuery,
            RenameRequest,
            ChmodRequest,
            TruncateRequest,
            SymlinkRequest,
            MetadataResponse,
            ActionResponse,
            XattrQuery,
            XattrListQuery,
            SetXattrRequest,
            XattrListResponse,
            XattrValueResponse,
        )
    ),
    tags(
        (name = "files", description = "File operations"),
        (name = "directories", description = "Directory operations"),
        (name = "metadata", description = "Metadata operations"),
        (name = "xattr", description = "Extended attribute operations"),
        (name = "health", description = "Health check endpoints"),
    ),
    info(
        title = "RACFS API",
        description = "Remote Agent Call File System API",
        version = "0.1.0",
    ),
)]
pub struct ApiDoc;

#[cfg(test)]
mod tests {
    use super::ApiDoc;
    use utoipa::OpenApi;

    #[test]
    fn openapi_spec_generates() {
        let spec = ApiDoc::openapi();
        let json = spec.to_pretty_json().unwrap();
        assert!(json.contains("/api/v1/files"));
        assert!(json.contains("openapi"));
    }
}
