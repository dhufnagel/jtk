//! The project configuration in `jtk.toml`: the nearest one in a file's directory or above applies to it. Paths in
//! it are relative to its directory. Command line options override it: `--select` replaces `select`, `--java`
//! replaces `java`, and `--extend-select` and `--ignore` add to the configured rules.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde::Deserialize;

use crate::formatter::FormatOptions;
use crate::lint::{RuleOptions, Selection, Settings};

pub const FILE_NAME: &str = "jtk.toml";

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
struct ConfigFile {
    /// The project's Java version.
    java: Option<u32>,
    /// Files and directories not to check when a directory is given.
    exclude: Vec<String>,
    format: FormatSection,
    lint: LintSection,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
struct FormatSection {
    sort_imports: Option<bool>,
    remove_unused_imports: Option<bool>,
    reflow_long_strings: Option<bool>,
    format_javadoc: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
struct LintSection {
    select: Vec<String>,
    extend_select: Vec<String>,
    ignore: Vec<String>,
    /// Rule code prefixes not to check in the files matching a pattern.
    per_file_ignores: BTreeMap<String, Vec<String>>,
    options: RuleOptions,
}

/// The rule selection given on the command line, which overrides the configured one.
#[derive(Debug, Default)]
pub struct LintArguments {
    pub select: Vec<String>,
    pub extend_select: Vec<String>,
    pub ignore: Vec<String>,
    pub java: Option<u32>,
}

/// A loaded configuration, or the defaults where there is none.
#[derive(Debug)]
pub struct Config {
    /// The directory the paths in the configuration are relative to.
    dir: PathBuf,
    file: ConfigFile,
    exclude: GlobSet,
    per_file_ignores: Vec<(GlobSet, Vec<String>)>,
}

impl Config {
    fn defaults() -> Config {
        let file = ConfigFile::default();
        Config { dir: PathBuf::new(), file, exclude: GlobSet::empty(), per_file_ignores: Vec::new() }
    }

    /// Reads and checks a configuration file; errors start with its path.
    pub fn load(path: &Path) -> Result<Config, String> {
        let error = |message: String| format!("{}: {message}", path.display());
        let text = std::fs::read_to_string(path).map_err(|e| error(e.to_string()))?;
        let file: ConfigFile = toml::from_str(&text).map_err(|e| error(e.to_string()))?;
        let lint = &file.lint;
        let codes = lint.per_file_ignores.values().flatten().cloned().collect();
        Selection::new(lint.select.clone(), lint.ignore.clone())
            .and_then(|s| s.extended(lint.extend_select.clone()))
            .and_then(|_| Selection::new(Vec::new(), codes))
            .map_err(error)?;
        let exclude = glob_set(&file.exclude).map_err(error)?;
        let per_file_ignores = lint
            .per_file_ignores
            .iter()
            .map(|(pattern, codes)| Ok((glob_set(std::slice::from_ref(pattern))?, codes.clone())))
            .collect::<Result<_, String>>()
            .map_err(error)?;
        let dir = std::path::absolute(path).map_err(|e| error(e.to_string()))?.parent().map(Path::to_path_buf);
        Ok(Config { dir: dir.unwrap_or_default(), file, exclude, per_file_ignores })
    }

    /// Whether a file found in a directory given on the command line is excluded.
    pub fn is_excluded(&self, file: &Path) -> bool {
        matches(&self.exclude, &self.relative(file))
    }

    /// The formatting steps: those switched off on the command line or in the configuration are skipped.
    pub fn format_options(&self, command_line: FormatOptions) -> FormatOptions {
        let format = &self.file.format;
        FormatOptions {
            sort_imports: command_line.sort_imports && format.sort_imports.unwrap_or(true),
            remove_unused_imports: command_line.remove_unused_imports && format.remove_unused_imports.unwrap_or(true),
            reflow_long_strings: command_line.reflow_long_strings && format.reflow_long_strings.unwrap_or(true),
            format_javadoc: command_line.format_javadoc || format.format_javadoc.unwrap_or(false),
            ..command_line
        }
    }

    /// The rules to check in a file and their parameters.
    pub fn lint_settings(&self, file: &Path, arguments: &LintArguments) -> Result<Settings, String> {
        let lint = &self.file.lint;
        let select = if arguments.select.is_empty() { &lint.select } else { &arguments.select };
        let relative = self.relative(file);
        let file_ignores = self.per_file_ignores.iter().filter(|(globs, _)| matches(globs, &relative));
        let ignore = lint.ignore.iter().chain(&arguments.ignore).chain(file_ignores.flat_map(|(_, codes)| codes));
        let extend = lint.extend_select.iter().chain(&arguments.extend_select);
        let selection =
            Selection::new(select.clone(), ignore.cloned().collect())?.extended(extend.cloned().collect())?;
        let selection = match arguments.java.or(self.file.java) {
            Some(version) => selection.with_java_version(version),
            None => selection,
        };
        Ok(Settings { selection, options: lint.options.clone() })
    }

    fn relative(&self, file: &Path) -> PathBuf {
        let file = std::path::absolute(file).unwrap_or_else(|_| file.to_path_buf());
        file.strip_prefix(&self.dir).map(Path::to_path_buf).unwrap_or(file)
    }
}

/// Patterns as in `.gitignore` files: `*` does not match `/`, and `**` matches any directories.
fn glob_set(patterns: &[String]) -> Result<GlobSet, String> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let pattern = pattern.trim_end_matches('/');
        let glob = GlobBuilder::new(pattern).literal_separator(true).build().map_err(|e| e.to_string())?;
        builder.add(glob);
    }
    builder.build().map_err(|e| e.to_string())
}

/// Whether the path or one of its directories matches, so that a pattern can name a directory.
fn matches(globs: &GlobSet, relative: &Path) -> bool {
    !globs.is_empty() && relative.ancestors().any(|p| !p.as_os_str().is_empty() && globs.is_match(p))
}

/// Finds the configuration of each file, loading each `jtk.toml` once.
pub struct Configs {
    /// The configuration given with `--config`, which applies to all files.
    given: Option<Arc<Config>>,
    defaults: Arc<Config>,
    by_dir: HashMap<PathBuf, Arc<Config>>,
}

impl Configs {
    pub fn new(given: Option<&Path>) -> Result<Configs, String> {
        let given = given.map(Config::load).transpose()?.map(Arc::new);
        Ok(Configs { given, defaults: Arc::new(Config::defaults()), by_dir: HashMap::new() })
    }

    /// The configuration that applies to a file: the given one, or the nearest in the file's directory or above.
    pub fn for_file(&mut self, file: &Path) -> Result<Arc<Config>, String> {
        if let Some(given) = &self.given {
            return Ok(given.clone());
        }
        let file = std::path::absolute(file).map_err(|e| format!("{}: {e}", file.display()))?;
        let dir = file.parent().unwrap_or(Path::new("/"));
        self.for_dir(dir)
    }

    fn for_dir(&mut self, dir: &Path) -> Result<Arc<Config>, String> {
        if let Some(config) = self.by_dir.get(dir) {
            return Ok(config.clone());
        }
        let path = dir.join(FILE_NAME);
        let config = if path.is_file() {
            Arc::new(Config::load(&path)?)
        } else {
            match dir.parent() {
                Some(parent) => self.for_dir(parent)?,
                None => self.defaults.clone(),
            }
        };
        self.by_dir.insert(dir.to_path_buf(), config.clone());
        Ok(config)
    }
}

/// The Java files to check for the paths given on the command line: files as given, and the `.java` files in
/// directories, except those ignored by `.gitignore` files or excluded by the configuration, ordered by path.
pub fn java_files(paths: &[String], configs: &mut Configs) -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    for path in paths {
        if !Path::new(path).is_dir() {
            files.push(path.clone());
            continue;
        }
        let mut found = Vec::new();
        for entry in ignore::WalkBuilder::new(path).build() {
            let entry = entry.map_err(|e| e.to_string())?;
            let is_java =
                entry.file_type().is_some_and(|t| t.is_file()) && entry.path().extension().is_some_and(|e| e == "java");
            if is_java && !configs.for_file(entry.path())?.is_excluded(entry.path()) {
                found.push(entry.path().to_string_lossy().into_owned());
            }
        }
        found.sort();
        files.extend(found);
    }
    Ok(files)
}
