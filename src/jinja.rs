use abscissa_core::Application;
use anyhow::Result;
use comfy_table::{
    Attribute, Cell, CellAlignment, ContentArrangement, Table, presets::ASCII_MARKDOWN,
};
use include_dir::{Dir, include_dir};
use jiff::SignedDuration;
use minijinja::{
    AutoEscape, Environment, UndefinedBehavior, Value, context, path_loader, syntax::SyntaxConfig,
    value::Serde,
};
use rustic_core::{
    CopySnapshot, ForgetGroups, Group,
    repofile::{KeyFile, KeyId, RusticTime, SnapshotFile},
};

use crate::{
    RUSTIC_APP,
    commands::{repoinfo::RepoInfos, snapshots::SnapshotCmd},
};

#[derive(Clone, Copy)]
pub(crate) enum Alignment {
    Left,
    Right,
    Center,
}

/// helper func for parsing a table from a string
/// rows are strictly separated by "---\n", prependended by whitespaces, first row is taken as header
/// columns are separated by |
/// Alignment of columns is set in the header using '<' and '>' , e.g. "| left | left <|> center <|> right |"
fn parse_table(value: &str) -> (Vec<&str>, Vec<Alignment>, impl Iterator<Item = Vec<&str>>) {
    let mut rows = value
        .split("---\n")
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| line.trim_matches('|').split('|').map(str::trim));

    // parse header and extract alignment
    let mut headers = Vec::new();
    let mut alignments = Vec::new();
    if let Some(header_cells) = rows.next() {
        for raw in header_cells {
            let (title, align) = match (raw.strip_prefix('>'), raw.strip_suffix('<')) {
                // "> Title <" -> centered
                (Some(rest), _) if rest.ends_with('<') => {
                    (rest.strip_suffix('<').unwrap(), Alignment::Center)
                }
                // "> Title" -> right-aligned
                (Some(rest), None) => (rest, Alignment::Right),
                // "Title <" -> left-aligned
                (None, Some(rest)) => (rest, Alignment::Left),
                // "Title" -> left-aligned (default)
                _ => (raw, Alignment::Left),
            };
            headers.push(title);
            alignments.push(align);
        }
    }
    (headers, alignments, rows.map(|r| r.collect()))
}

impl From<Alignment> for CellAlignment {
    fn from(val: Alignment) -> Self {
        match val {
            Alignment::Left => Self::Left,
            Alignment::Right => Self::Right,
            Alignment::Center => Self::Center,
        }
    }
}

// filter function for minijinja
fn table_filter(value: &str) -> String {
    let mut table = Table::new();
    _ = table
        .load_style(ASCII_MARKDOWN)
        .set_content_arrangement(ContentArrangement::Dynamic);

    let (headers, alignments, rows) = parse_table(value);
    let bold_cell = |s| Cell::new(s).add_attribute(Attribute::Bold);
    _ = table.set_header(headers.into_iter().map(bold_cell));
    // Apply alignment to columns
    for (idx, align) in alignments.into_iter().enumerate() {
        if let Some(col) = table.column_mut(idx) {
            col.set_cell_alignment(align.into());
        }
    }
    for row in rows {
        _ = table.add_row(row);
    }
    table.to_string()
}

fn timeformat(time: &str) -> Option<String> {
    let config = RUSTIC_APP.config();
    RusticTime::parse_system(time)
        .ok()
        .map(|t| config.global.format_time(&t).to_string())
}

fn durationformat(duration: f64) -> String {
    format!("{:#}", SignedDuration::from_secs_f64(duration))
}

static DEFAULT_TEMPLATES: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/jinja2_templates");

fn render_jinja(template: &str, value: impl Into<Value>) -> Result<String> {
    let config = RUSTIC_APP.config();
    let mut env = Environment::new();
    env.set_debug(true);
    env.set_auto_escape_callback(|_| AutoEscape::None);
    env.set_undefined_behavior(UndefinedBehavior::Chainable);
    env.set_syntax(
        SyntaxConfig::builder()
            .trim_blocks(true)
            .lstrip_blocks(true)
            .build()
            .unwrap(),
    );

    // add minijinja_contrib
    minijinja_contrib::add_to_environment(&mut env);

    // register custom filters
    env.add_filter("table", table_filter);
    env.add_filter("timeformat", timeformat);
    env.add_filter("durationformat", durationformat);

    // Load templates either from path or compiled-in-defaults
    let dir_loader = config.global.template_path.as_ref().map(path_loader);
    env.set_loader(move |name| {
        // first try to load from path
        if let Some(dir_loader) = dir_loader.as_ref()
            && let Some(source) = dir_loader(name)?
        {
            return Ok(Some(source));
        }

        // use default if not found from path
        if let Some(file) = DEFAULT_TEMPLATES.get_file(name)
            && let Some(contents) = file.contents_utf8()
        {
            return Ok(Some(contents.into()));
        }

        Ok(None)
    });

    Ok(env.get_template(template)?.render(value)?)
}

pub fn output_snapshots(groups: &Vec<Group<SnapshotFile>>, opts: &SnapshotCmd) -> Result<String> {
    render_jinja(
        "output_snapshots.j2",
        context!(snapshot_groups => Serde(groups), opts => Serde(opts)),
    )
}

pub fn output_backup(snap: &SnapshotFile) -> Result<String> {
    render_jinja("output_backup.j2", context!(snap => Serde(snap)))
}

pub fn output_forget(forget_groups: &ForgetGroups) -> Result<String> {
    render_jinja(
        "output_forget.j2",
        context!(forget_groups => Serde(forget_groups)),
    )
}

pub fn output_repoinfo(infos: &RepoInfos) -> Result<String> {
    render_jinja("output_repoinfo.j2", context!(infos => Serde(infos)))
}

pub fn output_rewrite(snaps: &Vec<SnapshotFile>) -> Result<String> {
    render_jinja("output_rewrite.j2", context!(snapshots => Serde(snaps)))
}

pub fn output_key(
    keys: impl Iterator<Item = (KeyId, KeyFile)>,
    used_key: &Option<KeyId>,
) -> Result<String> {
    render_jinja(
        "output_key.j2",
        context!(keys => Value::from_iter(keys.map(Serde)), used_key => Serde(used_key)),
    )
}

pub fn output_copy(snaps: &[CopySnapshot]) -> Result<String> {
    // TODO: impl Serialize for CopySnapshot in rustic_core
    let snaps: Vec<_> = snaps.iter().map(|sn| (&sn.sn, sn.relevant)).collect();
    render_jinja("output_copy.j2", context!(copy_snapshots => Serde(snaps)))
}

pub fn tui_snapshot_details(
    snap: &SnapshotFile,
    process_header: impl FnOnce(Vec<&str>),
    process_alignments: impl FnOnce(Vec<Alignment>),
    mut process_row: impl FnMut(Vec<&str>),
) -> Result<()> {
    let template = render_jinja(
        "table_tui_snapshot_details.j2",
        context!(snap => Serde(snap)),
    )?;
    let (headers, alignments, iter) = parse_table(&template);
    process_header(headers);
    process_alignments(alignments);
    for row in iter {
        process_row(row);
    }
    Ok(())
}

pub fn tui_snapshot_diff_details(
    snap1: &SnapshotFile,
    snap2: &SnapshotFile,
    process_header: impl FnOnce(Vec<&str>),
    process_alignments: impl FnOnce(Vec<Alignment>),
    mut process_row: impl FnMut(Vec<&str>),
) -> Result<()> {
    let template = render_jinja(
        "table_tui_snapshot_diff_details.j2",
        context!(snap1 => Serde(snap1), snap2 => Serde(snap2)),
    )?;
    let (headers, alignments, iter) = parse_table(&template);
    process_header(headers);
    process_alignments(alignments);
    for row in iter {
        process_row(row);
    }
    Ok(())
}
