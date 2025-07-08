use proton_sdk_sys::protobufs::{FileNode, FolderNode, NodeType};

pub fn node_is_folder(node: NodeType) -> (bool, Option<FolderNode>) {
    match node.node_type {
        Some(proton_sdk_sys::protobufs::node_type::NodeType::FolderNode(folder)) => (true, Some(folder)),
        _ => (false, None),
    }
}

pub fn node_is_file(node: NodeType) -> (bool, Option<FileNode>) {
    match node.node_type {
        Some(proton_sdk_sys::protobufs::node_type::NodeType::FileNode(file)) => (true, Some(file)),
        _ => (false, None),
    }
}