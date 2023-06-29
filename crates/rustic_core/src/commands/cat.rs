use std::path::Path;

use bytes::Bytes;

use crate::{
    error::CommandErrorKind,
    repository::{Indexed, Open, Repository},
    BlobType, DecryptReadBackend, FileType, Id, IndexedBackend, ProgressBars, ReadBackend,
    RusticResult, SnapshotFile, Tree,
};

pub(crate) fn cat_file<P, S: Open>(
    repo: &Repository<P, S>,
    tpe: FileType,
    id: &str,
) -> RusticResult<Bytes> {
    let id = repo.dbe().find_id(tpe, id)?;
    let data = repo.dbe().read_encrypted_full(tpe, &id)?;
    Ok(data)
}

pub(crate) fn cat_blob<P, S: Indexed>(
    repo: &Repository<P, S>,
    tpe: BlobType,
    id: &str,
) -> RusticResult<Bytes> {
    let id = Id::from_hex(id)?;
    let data = repo.index().blob_from_backend(tpe, &id)?;

    Ok(data)
}

pub(crate) fn cat_tree<P: ProgressBars, S: Indexed>(
    repo: &Repository<P, S>,
    snap: &str,
    sn_filter: impl FnMut(&SnapshotFile) -> bool + Send + Sync,
) -> RusticResult<Bytes> {
    let (id, path) = snap.split_once(':').unwrap_or((snap, ""));
    let snap = SnapshotFile::from_str(
        repo.dbe(),
        id,
        sn_filter,
        &repo.pb.progress_counter("getting snapshot..."),
    )?;
    let node = Tree::node_from_path(repo.index(), snap.tree, Path::new(path))?;
    let id = node
        .subtree
        .ok_or_else(|| CommandErrorKind::PathIsNoDir(path.to_string()))?;
    let data = repo.index().blob_from_backend(BlobType::Tree, &id)?;
    Ok(data)
}
