use std::path::Path;

use bytes::Bytes;

use crate::{
    error::CommandErrorKind, BlobType, DecryptReadBackend, FileType, Id, IndexBackend,
    IndexedBackend, OpenRepository, ProgressBars, ReadBackend, RusticResult, SnapshotFile, Tree,
};

pub fn cat_file(repo: &OpenRepository, tpe: FileType, id: &str) -> RusticResult<Bytes> {
    let id = repo.dbe.find_id(tpe, id)?;
    let data = repo.dbe.read_encrypted_full(tpe, &id)?;
    Ok(data)
}

pub fn cat_blob(
    repo: &OpenRepository,
    tpe: BlobType,
    id: &str,
    pb: &impl ProgressBars,
) -> RusticResult<Bytes> {
    let id = Id::from_hex(id)?;
    let data = IndexBackend::new(&repo.dbe, &pb.progress_hidden())?.blob_from_backend(tpe, &id)?;

    Ok(data)
}

pub fn cat_tree(
    repo: &OpenRepository,
    snap: &str,
    sn_filter: impl FnMut(&SnapshotFile) -> bool + Send + Sync,
    pb: &impl ProgressBars,
) -> RusticResult<Bytes> {
    let (id, path) = snap.split_once(':').unwrap_or((snap, ""));
    let snap = SnapshotFile::from_str(&repo.dbe, id, sn_filter, &pb.progress_counter(""))?;
    let index = IndexBackend::new(&repo.dbe, &pb.progress_counter(""))?;
    let node = Tree::node_from_path(&index, snap.tree, Path::new(path))?;
    let id = node
        .subtree
        .ok_or_else(|| CommandErrorKind::PathIsNoDir(path.to_string()))?;
    let data = index.blob_from_backend(BlobType::Tree, &id)?;
    Ok(data)
}
