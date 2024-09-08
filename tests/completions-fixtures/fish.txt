# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_rustic_global_optspecs
	string join \n P/use-profile= n/dry-run check-index log-level= log-file= no-progress progress-interval= r/repository= repo-hot= password= p/password-file= password-command= no-cache cache-dir= warm-up warm-up-command= warm-up-wait= filter-host= filter-label= filter-paths= filter-tags= filter-fn= h/help V/version
end

function __fish_rustic_needs_command
	# Figure out if the current invocation already has a command.
	set -l cmd (commandline -opc)
	set -e cmd[1]
	argparse -s (__fish_rustic_global_optspecs) -- $cmd 2>/dev/null
	or return
	if set -q argv[1]
		# Also print the command, so this can be used to figure out what it is.
		echo $argv[1]
		return 1
	end
	return 0
end

function __fish_rustic_using_subcommand
	set -l cmd (__fish_rustic_needs_command)
	test -z "$cmd"
	and return 1
	contains -- $cmd[1] $argv
end

complete -c rustic -n "__fish_rustic_needs_command" -s P -l use-profile -d 'Config profile to use. This parses the file `<PROFILE>.toml` in the config directory. [default: "rustic"]' -r
complete -c rustic -n "__fish_rustic_needs_command" -l log-level -d 'Use this log level [default: info]' -r
complete -c rustic -n "__fish_rustic_needs_command" -l log-file -d 'Write log messages to the given file instead of printing them' -r -F
complete -c rustic -n "__fish_rustic_needs_command" -l progress-interval -d 'Interval to update progress bars' -r
complete -c rustic -n "__fish_rustic_needs_command" -s r -l repository -l repo -d 'Repository to use' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_needs_command" -l repo-hot -d 'Repository to use as hot storage' -r
complete -c rustic -n "__fish_rustic_needs_command" -l password -d 'Password of the repository' -r
complete -c rustic -n "__fish_rustic_needs_command" -s p -l password-file -d 'File to read the password from' -r -F
complete -c rustic -n "__fish_rustic_needs_command" -l password-command -d 'Command to read the password from. Password is read from stdout' -r
complete -c rustic -n "__fish_rustic_needs_command" -l cache-dir -d 'Use this dir as cache dir instead of the standard cache dir' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_needs_command" -l warm-up-command -d 'Warm up needed data pack files by running the command with %id replaced by pack id' -r
complete -c rustic -n "__fish_rustic_needs_command" -l warm-up-wait -d 'Duration (e.g. 10m) to wait after warm up' -r
complete -c rustic -n "__fish_rustic_needs_command" -l filter-host -d 'Hostname to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_needs_command" -l filter-label -d 'Label to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_needs_command" -l filter-paths -d 'Path list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_needs_command" -l filter-tags -d 'Tag list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_needs_command" -l filter-fn -d 'Function to filter snapshots' -r
complete -c rustic -n "__fish_rustic_needs_command" -s n -l dry-run -d 'Only show what would be done without modifying anything. Does not affect read-only commands'
complete -c rustic -n "__fish_rustic_needs_command" -l check-index -d 'Check if index matches pack files and read pack headers if neccessary'
complete -c rustic -n "__fish_rustic_needs_command" -l no-progress -d 'Don\'t show any progress bar'
complete -c rustic -n "__fish_rustic_needs_command" -l no-cache -d 'Don\'t use a cache'
complete -c rustic -n "__fish_rustic_needs_command" -l warm-up -d 'Warm up needed data pack files by only requesting them without processing'
complete -c rustic -n "__fish_rustic_needs_command" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c rustic -n "__fish_rustic_needs_command" -s V -l version -d 'Print version'
complete -c rustic -n "__fish_rustic_needs_command" -f -a "backup" -d 'Backup to the repository'
complete -c rustic -n "__fish_rustic_needs_command" -f -a "cat" -d 'Show raw data of repository files and blobs'
complete -c rustic -n "__fish_rustic_needs_command" -f -a "config" -d 'Change the repository configuration'
complete -c rustic -n "__fish_rustic_needs_command" -f -a "completions" -d 'Generate shell completions'
complete -c rustic -n "__fish_rustic_needs_command" -f -a "check" -d 'Check the repository'
complete -c rustic -n "__fish_rustic_needs_command" -f -a "copy" -d 'Copy snapshots to other repositories. Note: The target repositories must be given in the config file!'
complete -c rustic -n "__fish_rustic_needs_command" -f -a "diff" -d 'Compare two snapshots/paths Note that the exclude options only apply for comparison with a local path'
complete -c rustic -n "__fish_rustic_needs_command" -f -a "dump" -d 'dump the contents of a file in a snapshot to stdout'
complete -c rustic -n "__fish_rustic_needs_command" -f -a "find" -d 'Find in given snapshots'
complete -c rustic -n "__fish_rustic_needs_command" -f -a "forget" -d 'Remove snapshots from the repository'
complete -c rustic -n "__fish_rustic_needs_command" -f -a "init" -d 'Initialize a new repository'
complete -c rustic -n "__fish_rustic_needs_command" -f -a "key" -d 'Manage keys'
complete -c rustic -n "__fish_rustic_needs_command" -f -a "list" -d 'List repository files'
complete -c rustic -n "__fish_rustic_needs_command" -f -a "ls" -d 'List file contents of a snapshot'
complete -c rustic -n "__fish_rustic_needs_command" -f -a "merge" -d 'Merge snapshots'
complete -c rustic -n "__fish_rustic_needs_command" -f -a "snapshots" -d 'Show a detailed overview of the snapshots within the repository'
complete -c rustic -n "__fish_rustic_needs_command" -f -a "show-config" -d 'Show the configuration which has been read from the config file(s)'
complete -c rustic -n "__fish_rustic_needs_command" -f -a "self-update" -d 'Update to the latest rustic release'
complete -c rustic -n "__fish_rustic_needs_command" -f -a "prune" -d 'Remove unused data or repack repository pack files'
complete -c rustic -n "__fish_rustic_needs_command" -f -a "restore" -d 'Restore a snapshot/path'
complete -c rustic -n "__fish_rustic_needs_command" -f -a "repair" -d 'Repair a snapshot/path'
complete -c rustic -n "__fish_rustic_needs_command" -f -a "repoinfo" -d 'Show general information about the repository'
complete -c rustic -n "__fish_rustic_needs_command" -f -a "tag" -d 'Change tags of snapshots'
complete -c rustic -n "__fish_rustic_needs_command" -f -a "webdav" -d 'Start a webdav server which allows to access the repository'
complete -c rustic -n "__fish_rustic_needs_command" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l stdin-filename -d 'Set filename to be used when backing up from stdin' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l as-path -d 'Manually set backup path in snapshot' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand backup" -s g -l group-by -d 'Group snapshots by any combination of host,label,paths,tags to find a suitable parent (default: host,label,paths)' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l parent -d 'Snapshot to use as parent' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l glob -d 'Glob pattern to exclude/include (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l iglob -d 'Same as --glob pattern but ignores the casing of filenames' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l glob-file -d 'Read glob patterns to exclude/include from this file (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l iglob-file -d 'Same as --glob-file ignores the casing of filenames in patterns' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l custom-ignorefile -d 'Treat the provided filename like a .gitignore file (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l exclude-if-present -d 'Exclude contents of directories containing this filename (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l exclude-larger-than -d 'Maximum size of files to be backed up. Larger files will be excluded' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l label -d 'Label snapshot with given label' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l tag -d 'Tags to add to snapshot (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l description -d 'Add description to snapshot' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l description-from -d 'Add description to snapshot from file' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l time -d 'Set the backup time manually' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l delete-after -d 'Mark snapshot to be deleted after given duration (e.g. 10d)' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l host -d 'Set the host name manually' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l command -d 'Set the backup command manually' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l hostname -d 'Set \'hostname\' in public key information' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l username -d 'Set \'username\' in public key information' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l set-compression -d 'Set compression level. Allowed levels are 1 to 22 and -1 to -7, see <https://facebook.github.io/zstd/>. Note that 0 equals to no compression' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l set-version -d 'Set repository version. Allowed versions: 1,2' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l set-append-only -d 'Set append-only mode. Note that only append-only commands work once this is set. `forget`, `prune` or `config` won\'t work any longer' -r -f -a "{true\t'',false\t''}"
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l set-treepack-size -d 'Set default packsize for tree packs. rustic tries to always produce packs greater than this value. Note that for large repos, this value is grown by the grown factor. Defaults to `4 MiB` if not set' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l set-treepack-size-limit -d 'Set upper limit for default packsize for tree packs. Note that packs actually can get a bit larger. If not set, pack sizes can grow up to approximately `4 GiB`' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l set-treepack-growfactor -d 'Set grow factor for tree packs. The default packsize grows by the square root of the total size of all tree packs multiplied with this factor. This means 32 kiB times this factor per square root of total treesize in GiB. Defaults to `32` (= 1MB per square root of total treesize in GiB) if not set' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l set-datapack-size -d 'Set default packsize for data packs. rustic tries to always produce packs greater than this value. Note that for large repos, this value is grown by the grown factor. Defaults to `32 MiB` if not set' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l set-datapack-growfactor -d 'Set grow factor for data packs. The default packsize grows by the square root of the total size of all data packs multiplied with this factor. This means 32 kiB times this factor per square root of total datasize in GiB. Defaults to `32` (= 1MB per square root of total datasize in GiB) if not set' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l set-datapack-size-limit -d 'Set upper limit for default packsize for tree packs. Note that packs actually can get a bit larger. If not set, pack sizes can grow up to approximately `4 GiB`' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l set-min-packsize-tolerate-percent -d 'Set minimum tolerated packsize in percent of the targeted packsize. Defaults to `30` if not set' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l set-max-packsize-tolerate-percent -d 'Set maximum tolerated packsize in percent of the targeted packsize A value of `0` means packs larger than the targeted packsize are always tolerated. Default if not set: larger packfiles are always tolerated' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l set-extra-verify -d 'Do an extra verification by decompressing/decrypting all data before uploading to the repository. Default: true' -r -f -a "{true\t'',false\t''}"
complete -c rustic -n "__fish_rustic_using_subcommand backup" -s P -l use-profile -d 'Config profile to use. This parses the file `<PROFILE>.toml` in the config directory. [default: "rustic"]' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l log-level -d 'Use this log level [default: info]' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l log-file -d 'Write log messages to the given file instead of printing them' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l progress-interval -d 'Interval to update progress bars' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -s r -l repository -l repo -d 'Repository to use' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l repo-hot -d 'Repository to use as hot storage' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l password -d 'Password of the repository' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -s p -l password-file -d 'File to read the password from' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l password-command -d 'Command to read the password from. Password is read from stdout' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l cache-dir -d 'Use this dir as cache dir instead of the standard cache dir' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l warm-up-command -d 'Warm up needed data pack files by running the command with %id replaced by pack id' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l warm-up-wait -d 'Duration (e.g. 10m) to wait after warm up' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l filter-host -d 'Hostname to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l filter-label -d 'Label to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l filter-paths -d 'Path list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l filter-tags -d 'Tag list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l filter-fn -d 'Function to filter snapshots' -r
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l with-atime -d 'Save access time for files and directories'
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l ignore-devid -d 'Don\'t save device ID for files and directories'
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l no-scan -d 'Don\'t scan the backup source for its size - this disables ETA estimation for backup'
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l json -d 'Output generated snapshot in json format'
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l quiet -d 'Don\'t show any output'
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l init -d 'Initialize repository, if it doesn\'t exist yet'
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l skip-identical-parent -d 'Skip writing of snapshot if nothing changed w.r.t. the parent snapshot'
complete -c rustic -n "__fish_rustic_using_subcommand backup" -s f -l force -d 'Use no parent, read all files'
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l ignore-ctime -d 'Ignore ctime changes when checking for modified files'
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l ignore-inode -d 'Ignore inode number changes when checking for modified files'
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l git-ignore -d 'Ignore files based on .gitignore files'
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l no-require-git -d 'Do not require a git repository to apply git-ignore rule'
complete -c rustic -n "__fish_rustic_using_subcommand backup" -s x -l one-file-system -d 'Exclude other file systems, don\'t cross filesystem boundaries and subvolumes'
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l delete-never -d 'Mark snapshot as uneraseable'
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l with-created -d 'Add \'created\' date in public key information'
complete -c rustic -n "__fish_rustic_using_subcommand backup" -s n -l dry-run -d 'Only show what would be done without modifying anything. Does not affect read-only commands'
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l check-index -d 'Check if index matches pack files and read pack headers if neccessary'
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l no-progress -d 'Don\'t show any progress bar'
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l no-cache -d 'Don\'t use a cache'
complete -c rustic -n "__fish_rustic_using_subcommand backup" -l warm-up -d 'Warm up needed data pack files by only requesting them without processing'
complete -c rustic -n "__fish_rustic_using_subcommand backup" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and not __fish_seen_subcommand_from tree-blob data-blob config index snapshot tree help" -s P -l use-profile -d 'Config profile to use. This parses the file `<PROFILE>.toml` in the config directory. [default: "rustic"]' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and not __fish_seen_subcommand_from tree-blob data-blob config index snapshot tree help" -l log-level -d 'Use this log level [default: info]' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and not __fish_seen_subcommand_from tree-blob data-blob config index snapshot tree help" -l log-file -d 'Write log messages to the given file instead of printing them' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand cat; and not __fish_seen_subcommand_from tree-blob data-blob config index snapshot tree help" -l progress-interval -d 'Interval to update progress bars' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and not __fish_seen_subcommand_from tree-blob data-blob config index snapshot tree help" -s r -l repository -l repo -d 'Repository to use' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand cat; and not __fish_seen_subcommand_from tree-blob data-blob config index snapshot tree help" -l repo-hot -d 'Repository to use as hot storage' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and not __fish_seen_subcommand_from tree-blob data-blob config index snapshot tree help" -l password -d 'Password of the repository' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and not __fish_seen_subcommand_from tree-blob data-blob config index snapshot tree help" -s p -l password-file -d 'File to read the password from' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand cat; and not __fish_seen_subcommand_from tree-blob data-blob config index snapshot tree help" -l password-command -d 'Command to read the password from. Password is read from stdout' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and not __fish_seen_subcommand_from tree-blob data-blob config index snapshot tree help" -l cache-dir -d 'Use this dir as cache dir instead of the standard cache dir' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand cat; and not __fish_seen_subcommand_from tree-blob data-blob config index snapshot tree help" -l warm-up-command -d 'Warm up needed data pack files by running the command with %id replaced by pack id' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and not __fish_seen_subcommand_from tree-blob data-blob config index snapshot tree help" -l warm-up-wait -d 'Duration (e.g. 10m) to wait after warm up' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and not __fish_seen_subcommand_from tree-blob data-blob config index snapshot tree help" -l filter-host -d 'Hostname to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and not __fish_seen_subcommand_from tree-blob data-blob config index snapshot tree help" -l filter-label -d 'Label to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and not __fish_seen_subcommand_from tree-blob data-blob config index snapshot tree help" -l filter-paths -d 'Path list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and not __fish_seen_subcommand_from tree-blob data-blob config index snapshot tree help" -l filter-tags -d 'Tag list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and not __fish_seen_subcommand_from tree-blob data-blob config index snapshot tree help" -l filter-fn -d 'Function to filter snapshots' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and not __fish_seen_subcommand_from tree-blob data-blob config index snapshot tree help" -s n -l dry-run -d 'Only show what would be done without modifying anything. Does not affect read-only commands'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and not __fish_seen_subcommand_from tree-blob data-blob config index snapshot tree help" -l check-index -d 'Check if index matches pack files and read pack headers if neccessary'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and not __fish_seen_subcommand_from tree-blob data-blob config index snapshot tree help" -l no-progress -d 'Don\'t show any progress bar'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and not __fish_seen_subcommand_from tree-blob data-blob config index snapshot tree help" -l no-cache -d 'Don\'t use a cache'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and not __fish_seen_subcommand_from tree-blob data-blob config index snapshot tree help" -l warm-up -d 'Warm up needed data pack files by only requesting them without processing'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and not __fish_seen_subcommand_from tree-blob data-blob config index snapshot tree help" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and not __fish_seen_subcommand_from tree-blob data-blob config index snapshot tree help" -f -a "tree-blob" -d 'Display a tree blob'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and not __fish_seen_subcommand_from tree-blob data-blob config index snapshot tree help" -f -a "data-blob" -d 'Display a data blob'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and not __fish_seen_subcommand_from tree-blob data-blob config index snapshot tree help" -f -a "config" -d 'Display the config file'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and not __fish_seen_subcommand_from tree-blob data-blob config index snapshot tree help" -f -a "index" -d 'Display an index file'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and not __fish_seen_subcommand_from tree-blob data-blob config index snapshot tree help" -f -a "snapshot" -d 'Display a snapshot file'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and not __fish_seen_subcommand_from tree-blob data-blob config index snapshot tree help" -f -a "tree" -d 'Display a tree within a snapshot'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and not __fish_seen_subcommand_from tree-blob data-blob config index snapshot tree help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree-blob" -s P -l use-profile -d 'Config profile to use. This parses the file `<PROFILE>.toml` in the config directory. [default: "rustic"]' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree-blob" -l log-level -d 'Use this log level [default: info]' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree-blob" -l log-file -d 'Write log messages to the given file instead of printing them' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree-blob" -l progress-interval -d 'Interval to update progress bars' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree-blob" -s r -l repository -l repo -d 'Repository to use' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree-blob" -l repo-hot -d 'Repository to use as hot storage' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree-blob" -l password -d 'Password of the repository' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree-blob" -s p -l password-file -d 'File to read the password from' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree-blob" -l password-command -d 'Command to read the password from. Password is read from stdout' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree-blob" -l cache-dir -d 'Use this dir as cache dir instead of the standard cache dir' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree-blob" -l warm-up-command -d 'Warm up needed data pack files by running the command with %id replaced by pack id' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree-blob" -l warm-up-wait -d 'Duration (e.g. 10m) to wait after warm up' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree-blob" -l filter-host -d 'Hostname to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree-blob" -l filter-label -d 'Label to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree-blob" -l filter-paths -d 'Path list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree-blob" -l filter-tags -d 'Tag list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree-blob" -l filter-fn -d 'Function to filter snapshots' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree-blob" -s n -l dry-run -d 'Only show what would be done without modifying anything. Does not affect read-only commands'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree-blob" -l check-index -d 'Check if index matches pack files and read pack headers if neccessary'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree-blob" -l no-progress -d 'Don\'t show any progress bar'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree-blob" -l no-cache -d 'Don\'t use a cache'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree-blob" -l warm-up -d 'Warm up needed data pack files by only requesting them without processing'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree-blob" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from data-blob" -s P -l use-profile -d 'Config profile to use. This parses the file `<PROFILE>.toml` in the config directory. [default: "rustic"]' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from data-blob" -l log-level -d 'Use this log level [default: info]' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from data-blob" -l log-file -d 'Write log messages to the given file instead of printing them' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from data-blob" -l progress-interval -d 'Interval to update progress bars' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from data-blob" -s r -l repository -l repo -d 'Repository to use' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from data-blob" -l repo-hot -d 'Repository to use as hot storage' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from data-blob" -l password -d 'Password of the repository' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from data-blob" -s p -l password-file -d 'File to read the password from' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from data-blob" -l password-command -d 'Command to read the password from. Password is read from stdout' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from data-blob" -l cache-dir -d 'Use this dir as cache dir instead of the standard cache dir' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from data-blob" -l warm-up-command -d 'Warm up needed data pack files by running the command with %id replaced by pack id' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from data-blob" -l warm-up-wait -d 'Duration (e.g. 10m) to wait after warm up' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from data-blob" -l filter-host -d 'Hostname to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from data-blob" -l filter-label -d 'Label to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from data-blob" -l filter-paths -d 'Path list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from data-blob" -l filter-tags -d 'Tag list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from data-blob" -l filter-fn -d 'Function to filter snapshots' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from data-blob" -s n -l dry-run -d 'Only show what would be done without modifying anything. Does not affect read-only commands'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from data-blob" -l check-index -d 'Check if index matches pack files and read pack headers if neccessary'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from data-blob" -l no-progress -d 'Don\'t show any progress bar'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from data-blob" -l no-cache -d 'Don\'t use a cache'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from data-blob" -l warm-up -d 'Warm up needed data pack files by only requesting them without processing'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from data-blob" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from config" -s P -l use-profile -d 'Config profile to use. This parses the file `<PROFILE>.toml` in the config directory. [default: "rustic"]' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from config" -l log-level -d 'Use this log level [default: info]' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from config" -l log-file -d 'Write log messages to the given file instead of printing them' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from config" -l progress-interval -d 'Interval to update progress bars' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from config" -s r -l repository -l repo -d 'Repository to use' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from config" -l repo-hot -d 'Repository to use as hot storage' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from config" -l password -d 'Password of the repository' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from config" -s p -l password-file -d 'File to read the password from' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from config" -l password-command -d 'Command to read the password from. Password is read from stdout' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from config" -l cache-dir -d 'Use this dir as cache dir instead of the standard cache dir' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from config" -l warm-up-command -d 'Warm up needed data pack files by running the command with %id replaced by pack id' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from config" -l warm-up-wait -d 'Duration (e.g. 10m) to wait after warm up' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from config" -l filter-host -d 'Hostname to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from config" -l filter-label -d 'Label to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from config" -l filter-paths -d 'Path list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from config" -l filter-tags -d 'Tag list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from config" -l filter-fn -d 'Function to filter snapshots' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from config" -s n -l dry-run -d 'Only show what would be done without modifying anything. Does not affect read-only commands'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from config" -l check-index -d 'Check if index matches pack files and read pack headers if neccessary'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from config" -l no-progress -d 'Don\'t show any progress bar'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from config" -l no-cache -d 'Don\'t use a cache'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from config" -l warm-up -d 'Warm up needed data pack files by only requesting them without processing'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from config" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from index" -s P -l use-profile -d 'Config profile to use. This parses the file `<PROFILE>.toml` in the config directory. [default: "rustic"]' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from index" -l log-level -d 'Use this log level [default: info]' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from index" -l log-file -d 'Write log messages to the given file instead of printing them' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from index" -l progress-interval -d 'Interval to update progress bars' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from index" -s r -l repository -l repo -d 'Repository to use' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from index" -l repo-hot -d 'Repository to use as hot storage' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from index" -l password -d 'Password of the repository' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from index" -s p -l password-file -d 'File to read the password from' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from index" -l password-command -d 'Command to read the password from. Password is read from stdout' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from index" -l cache-dir -d 'Use this dir as cache dir instead of the standard cache dir' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from index" -l warm-up-command -d 'Warm up needed data pack files by running the command with %id replaced by pack id' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from index" -l warm-up-wait -d 'Duration (e.g. 10m) to wait after warm up' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from index" -l filter-host -d 'Hostname to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from index" -l filter-label -d 'Label to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from index" -l filter-paths -d 'Path list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from index" -l filter-tags -d 'Tag list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from index" -l filter-fn -d 'Function to filter snapshots' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from index" -s n -l dry-run -d 'Only show what would be done without modifying anything. Does not affect read-only commands'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from index" -l check-index -d 'Check if index matches pack files and read pack headers if neccessary'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from index" -l no-progress -d 'Don\'t show any progress bar'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from index" -l no-cache -d 'Don\'t use a cache'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from index" -l warm-up -d 'Warm up needed data pack files by only requesting them without processing'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from index" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from snapshot" -s P -l use-profile -d 'Config profile to use. This parses the file `<PROFILE>.toml` in the config directory. [default: "rustic"]' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from snapshot" -l log-level -d 'Use this log level [default: info]' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from snapshot" -l log-file -d 'Write log messages to the given file instead of printing them' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from snapshot" -l progress-interval -d 'Interval to update progress bars' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from snapshot" -s r -l repository -l repo -d 'Repository to use' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from snapshot" -l repo-hot -d 'Repository to use as hot storage' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from snapshot" -l password -d 'Password of the repository' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from snapshot" -s p -l password-file -d 'File to read the password from' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from snapshot" -l password-command -d 'Command to read the password from. Password is read from stdout' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from snapshot" -l cache-dir -d 'Use this dir as cache dir instead of the standard cache dir' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from snapshot" -l warm-up-command -d 'Warm up needed data pack files by running the command with %id replaced by pack id' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from snapshot" -l warm-up-wait -d 'Duration (e.g. 10m) to wait after warm up' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from snapshot" -l filter-host -d 'Hostname to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from snapshot" -l filter-label -d 'Label to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from snapshot" -l filter-paths -d 'Path list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from snapshot" -l filter-tags -d 'Tag list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from snapshot" -l filter-fn -d 'Function to filter snapshots' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from snapshot" -s n -l dry-run -d 'Only show what would be done without modifying anything. Does not affect read-only commands'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from snapshot" -l check-index -d 'Check if index matches pack files and read pack headers if neccessary'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from snapshot" -l no-progress -d 'Don\'t show any progress bar'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from snapshot" -l no-cache -d 'Don\'t use a cache'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from snapshot" -l warm-up -d 'Warm up needed data pack files by only requesting them without processing'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from snapshot" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree" -s P -l use-profile -d 'Config profile to use. This parses the file `<PROFILE>.toml` in the config directory. [default: "rustic"]' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree" -l log-level -d 'Use this log level [default: info]' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree" -l log-file -d 'Write log messages to the given file instead of printing them' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree" -l progress-interval -d 'Interval to update progress bars' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree" -s r -l repository -l repo -d 'Repository to use' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree" -l repo-hot -d 'Repository to use as hot storage' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree" -l password -d 'Password of the repository' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree" -s p -l password-file -d 'File to read the password from' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree" -l password-command -d 'Command to read the password from. Password is read from stdout' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree" -l cache-dir -d 'Use this dir as cache dir instead of the standard cache dir' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree" -l warm-up-command -d 'Warm up needed data pack files by running the command with %id replaced by pack id' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree" -l warm-up-wait -d 'Duration (e.g. 10m) to wait after warm up' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree" -l filter-host -d 'Hostname to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree" -l filter-label -d 'Label to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree" -l filter-paths -d 'Path list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree" -l filter-tags -d 'Tag list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree" -l filter-fn -d 'Function to filter snapshots' -r
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree" -s n -l dry-run -d 'Only show what would be done without modifying anything. Does not affect read-only commands'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree" -l check-index -d 'Check if index matches pack files and read pack headers if neccessary'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree" -l no-progress -d 'Don\'t show any progress bar'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree" -l no-cache -d 'Don\'t use a cache'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree" -l warm-up -d 'Warm up needed data pack files by only requesting them without processing'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from tree" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from help" -f -a "tree-blob" -d 'Display a tree blob'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from help" -f -a "data-blob" -d 'Display a data blob'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from help" -f -a "config" -d 'Display the config file'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from help" -f -a "index" -d 'Display an index file'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from help" -f -a "snapshot" -d 'Display a snapshot file'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from help" -f -a "tree" -d 'Display a tree within a snapshot'
complete -c rustic -n "__fish_rustic_using_subcommand cat; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rustic -n "__fish_rustic_using_subcommand config" -l set-compression -d 'Set compression level. Allowed levels are 1 to 22 and -1 to -7, see <https://facebook.github.io/zstd/>. Note that 0 equals to no compression' -r
complete -c rustic -n "__fish_rustic_using_subcommand config" -l set-version -d 'Set repository version. Allowed versions: 1,2' -r
complete -c rustic -n "__fish_rustic_using_subcommand config" -l set-append-only -d 'Set append-only mode. Note that only append-only commands work once this is set. `forget`, `prune` or `config` won\'t work any longer' -r -f -a "{true\t'',false\t''}"
complete -c rustic -n "__fish_rustic_using_subcommand config" -l set-treepack-size -d 'Set default packsize for tree packs. rustic tries to always produce packs greater than this value. Note that for large repos, this value is grown by the grown factor. Defaults to `4 MiB` if not set' -r
complete -c rustic -n "__fish_rustic_using_subcommand config" -l set-treepack-size-limit -d 'Set upper limit for default packsize for tree packs. Note that packs actually can get a bit larger. If not set, pack sizes can grow up to approximately `4 GiB`' -r
complete -c rustic -n "__fish_rustic_using_subcommand config" -l set-treepack-growfactor -d 'Set grow factor for tree packs. The default packsize grows by the square root of the total size of all tree packs multiplied with this factor. This means 32 kiB times this factor per square root of total treesize in GiB. Defaults to `32` (= 1MB per square root of total treesize in GiB) if not set' -r
complete -c rustic -n "__fish_rustic_using_subcommand config" -l set-datapack-size -d 'Set default packsize for data packs. rustic tries to always produce packs greater than this value. Note that for large repos, this value is grown by the grown factor. Defaults to `32 MiB` if not set' -r
complete -c rustic -n "__fish_rustic_using_subcommand config" -l set-datapack-growfactor -d 'Set grow factor for data packs. The default packsize grows by the square root of the total size of all data packs multiplied with this factor. This means 32 kiB times this factor per square root of total datasize in GiB. Defaults to `32` (= 1MB per square root of total datasize in GiB) if not set' -r
complete -c rustic -n "__fish_rustic_using_subcommand config" -l set-datapack-size-limit -d 'Set upper limit for default packsize for tree packs. Note that packs actually can get a bit larger. If not set, pack sizes can grow up to approximately `4 GiB`' -r
complete -c rustic -n "__fish_rustic_using_subcommand config" -l set-min-packsize-tolerate-percent -d 'Set minimum tolerated packsize in percent of the targeted packsize. Defaults to `30` if not set' -r
complete -c rustic -n "__fish_rustic_using_subcommand config" -l set-max-packsize-tolerate-percent -d 'Set maximum tolerated packsize in percent of the targeted packsize A value of `0` means packs larger than the targeted packsize are always tolerated. Default if not set: larger packfiles are always tolerated' -r
complete -c rustic -n "__fish_rustic_using_subcommand config" -l set-extra-verify -d 'Do an extra verification by decompressing/decrypting all data before uploading to the repository. Default: true' -r -f -a "{true\t'',false\t''}"
complete -c rustic -n "__fish_rustic_using_subcommand config" -s P -l use-profile -d 'Config profile to use. This parses the file `<PROFILE>.toml` in the config directory. [default: "rustic"]' -r
complete -c rustic -n "__fish_rustic_using_subcommand config" -l log-level -d 'Use this log level [default: info]' -r
complete -c rustic -n "__fish_rustic_using_subcommand config" -l log-file -d 'Write log messages to the given file instead of printing them' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand config" -l progress-interval -d 'Interval to update progress bars' -r
complete -c rustic -n "__fish_rustic_using_subcommand config" -s r -l repository -l repo -d 'Repository to use' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand config" -l repo-hot -d 'Repository to use as hot storage' -r
complete -c rustic -n "__fish_rustic_using_subcommand config" -l password -d 'Password of the repository' -r
complete -c rustic -n "__fish_rustic_using_subcommand config" -s p -l password-file -d 'File to read the password from' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand config" -l password-command -d 'Command to read the password from. Password is read from stdout' -r
complete -c rustic -n "__fish_rustic_using_subcommand config" -l cache-dir -d 'Use this dir as cache dir instead of the standard cache dir' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand config" -l warm-up-command -d 'Warm up needed data pack files by running the command with %id replaced by pack id' -r
complete -c rustic -n "__fish_rustic_using_subcommand config" -l warm-up-wait -d 'Duration (e.g. 10m) to wait after warm up' -r
complete -c rustic -n "__fish_rustic_using_subcommand config" -l filter-host -d 'Hostname to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand config" -l filter-label -d 'Label to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand config" -l filter-paths -d 'Path list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand config" -l filter-tags -d 'Tag list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand config" -l filter-fn -d 'Function to filter snapshots' -r
complete -c rustic -n "__fish_rustic_using_subcommand config" -s n -l dry-run -d 'Only show what would be done without modifying anything. Does not affect read-only commands'
complete -c rustic -n "__fish_rustic_using_subcommand config" -l check-index -d 'Check if index matches pack files and read pack headers if neccessary'
complete -c rustic -n "__fish_rustic_using_subcommand config" -l no-progress -d 'Don\'t show any progress bar'
complete -c rustic -n "__fish_rustic_using_subcommand config" -l no-cache -d 'Don\'t use a cache'
complete -c rustic -n "__fish_rustic_using_subcommand config" -l warm-up -d 'Warm up needed data pack files by only requesting them without processing'
complete -c rustic -n "__fish_rustic_using_subcommand config" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c rustic -n "__fish_rustic_using_subcommand completions" -s P -l use-profile -d 'Config profile to use. This parses the file `<PROFILE>.toml` in the config directory. [default: "rustic"]' -r
complete -c rustic -n "__fish_rustic_using_subcommand completions" -l log-level -d 'Use this log level [default: info]' -r
complete -c rustic -n "__fish_rustic_using_subcommand completions" -l log-file -d 'Write log messages to the given file instead of printing them' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand completions" -l progress-interval -d 'Interval to update progress bars' -r
complete -c rustic -n "__fish_rustic_using_subcommand completions" -s r -l repository -l repo -d 'Repository to use' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand completions" -l repo-hot -d 'Repository to use as hot storage' -r
complete -c rustic -n "__fish_rustic_using_subcommand completions" -l password -d 'Password of the repository' -r
complete -c rustic -n "__fish_rustic_using_subcommand completions" -s p -l password-file -d 'File to read the password from' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand completions" -l password-command -d 'Command to read the password from. Password is read from stdout' -r
complete -c rustic -n "__fish_rustic_using_subcommand completions" -l cache-dir -d 'Use this dir as cache dir instead of the standard cache dir' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand completions" -l warm-up-command -d 'Warm up needed data pack files by running the command with %id replaced by pack id' -r
complete -c rustic -n "__fish_rustic_using_subcommand completions" -l warm-up-wait -d 'Duration (e.g. 10m) to wait after warm up' -r
complete -c rustic -n "__fish_rustic_using_subcommand completions" -l filter-host -d 'Hostname to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand completions" -l filter-label -d 'Label to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand completions" -l filter-paths -d 'Path list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand completions" -l filter-tags -d 'Tag list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand completions" -l filter-fn -d 'Function to filter snapshots' -r
complete -c rustic -n "__fish_rustic_using_subcommand completions" -s n -l dry-run -d 'Only show what would be done without modifying anything. Does not affect read-only commands'
complete -c rustic -n "__fish_rustic_using_subcommand completions" -l check-index -d 'Check if index matches pack files and read pack headers if neccessary'
complete -c rustic -n "__fish_rustic_using_subcommand completions" -l no-progress -d 'Don\'t show any progress bar'
complete -c rustic -n "__fish_rustic_using_subcommand completions" -l no-cache -d 'Don\'t use a cache'
complete -c rustic -n "__fish_rustic_using_subcommand completions" -l warm-up -d 'Warm up needed data pack files by only requesting them without processing'
complete -c rustic -n "__fish_rustic_using_subcommand completions" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c rustic -n "__fish_rustic_using_subcommand check" -s P -l use-profile -d 'Config profile to use. This parses the file `<PROFILE>.toml` in the config directory. [default: "rustic"]' -r
complete -c rustic -n "__fish_rustic_using_subcommand check" -l log-level -d 'Use this log level [default: info]' -r
complete -c rustic -n "__fish_rustic_using_subcommand check" -l log-file -d 'Write log messages to the given file instead of printing them' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand check" -l progress-interval -d 'Interval to update progress bars' -r
complete -c rustic -n "__fish_rustic_using_subcommand check" -s r -l repository -l repo -d 'Repository to use' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand check" -l repo-hot -d 'Repository to use as hot storage' -r
complete -c rustic -n "__fish_rustic_using_subcommand check" -l password -d 'Password of the repository' -r
complete -c rustic -n "__fish_rustic_using_subcommand check" -s p -l password-file -d 'File to read the password from' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand check" -l password-command -d 'Command to read the password from. Password is read from stdout' -r
complete -c rustic -n "__fish_rustic_using_subcommand check" -l cache-dir -d 'Use this dir as cache dir instead of the standard cache dir' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand check" -l warm-up-command -d 'Warm up needed data pack files by running the command with %id replaced by pack id' -r
complete -c rustic -n "__fish_rustic_using_subcommand check" -l warm-up-wait -d 'Duration (e.g. 10m) to wait after warm up' -r
complete -c rustic -n "__fish_rustic_using_subcommand check" -l filter-host -d 'Hostname to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand check" -l filter-label -d 'Label to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand check" -l filter-paths -d 'Path list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand check" -l filter-tags -d 'Tag list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand check" -l filter-fn -d 'Function to filter snapshots' -r
complete -c rustic -n "__fish_rustic_using_subcommand check" -l trust-cache -d 'Don\'t verify the data saved in the cache'
complete -c rustic -n "__fish_rustic_using_subcommand check" -l read-data -d 'Read all data blobs'
complete -c rustic -n "__fish_rustic_using_subcommand check" -s n -l dry-run -d 'Only show what would be done without modifying anything. Does not affect read-only commands'
complete -c rustic -n "__fish_rustic_using_subcommand check" -l check-index -d 'Check if index matches pack files and read pack headers if neccessary'
complete -c rustic -n "__fish_rustic_using_subcommand check" -l no-progress -d 'Don\'t show any progress bar'
complete -c rustic -n "__fish_rustic_using_subcommand check" -l no-cache -d 'Don\'t use a cache'
complete -c rustic -n "__fish_rustic_using_subcommand check" -l warm-up -d 'Warm up needed data pack files by only requesting them without processing'
complete -c rustic -n "__fish_rustic_using_subcommand check" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c rustic -n "__fish_rustic_using_subcommand copy" -l hostname -d 'Set \'hostname\' in public key information' -r
complete -c rustic -n "__fish_rustic_using_subcommand copy" -l username -d 'Set \'username\' in public key information' -r
complete -c rustic -n "__fish_rustic_using_subcommand copy" -s P -l use-profile -d 'Config profile to use. This parses the file `<PROFILE>.toml` in the config directory. [default: "rustic"]' -r
complete -c rustic -n "__fish_rustic_using_subcommand copy" -l log-level -d 'Use this log level [default: info]' -r
complete -c rustic -n "__fish_rustic_using_subcommand copy" -l log-file -d 'Write log messages to the given file instead of printing them' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand copy" -l progress-interval -d 'Interval to update progress bars' -r
complete -c rustic -n "__fish_rustic_using_subcommand copy" -s r -l repository -l repo -d 'Repository to use' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand copy" -l repo-hot -d 'Repository to use as hot storage' -r
complete -c rustic -n "__fish_rustic_using_subcommand copy" -l password -d 'Password of the repository' -r
complete -c rustic -n "__fish_rustic_using_subcommand copy" -s p -l password-file -d 'File to read the password from' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand copy" -l password-command -d 'Command to read the password from. Password is read from stdout' -r
complete -c rustic -n "__fish_rustic_using_subcommand copy" -l cache-dir -d 'Use this dir as cache dir instead of the standard cache dir' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand copy" -l warm-up-command -d 'Warm up needed data pack files by running the command with %id replaced by pack id' -r
complete -c rustic -n "__fish_rustic_using_subcommand copy" -l warm-up-wait -d 'Duration (e.g. 10m) to wait after warm up' -r
complete -c rustic -n "__fish_rustic_using_subcommand copy" -l filter-host -d 'Hostname to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand copy" -l filter-label -d 'Label to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand copy" -l filter-paths -d 'Path list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand copy" -l filter-tags -d 'Tag list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand copy" -l filter-fn -d 'Function to filter snapshots' -r
complete -c rustic -n "__fish_rustic_using_subcommand copy" -l init -d 'Initialize non-existing target repositories'
complete -c rustic -n "__fish_rustic_using_subcommand copy" -l with-created -d 'Add \'created\' date in public key information'
complete -c rustic -n "__fish_rustic_using_subcommand copy" -s n -l dry-run -d 'Only show what would be done without modifying anything. Does not affect read-only commands'
complete -c rustic -n "__fish_rustic_using_subcommand copy" -l check-index -d 'Check if index matches pack files and read pack headers if neccessary'
complete -c rustic -n "__fish_rustic_using_subcommand copy" -l no-progress -d 'Don\'t show any progress bar'
complete -c rustic -n "__fish_rustic_using_subcommand copy" -l no-cache -d 'Don\'t use a cache'
complete -c rustic -n "__fish_rustic_using_subcommand copy" -l warm-up -d 'Warm up needed data pack files by only requesting them without processing'
complete -c rustic -n "__fish_rustic_using_subcommand copy" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c rustic -n "__fish_rustic_using_subcommand diff" -l glob -d 'Glob pattern to exclude/include (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand diff" -l iglob -d 'Same as --glob pattern but ignores the casing of filenames' -r
complete -c rustic -n "__fish_rustic_using_subcommand diff" -l glob-file -d 'Read glob patterns to exclude/include from this file (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand diff" -l iglob-file -d 'Same as --glob-file ignores the casing of filenames in patterns' -r
complete -c rustic -n "__fish_rustic_using_subcommand diff" -l custom-ignorefile -d 'Treat the provided filename like a .gitignore file (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand diff" -l exclude-if-present -d 'Exclude contents of directories containing this filename (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand diff" -l exclude-larger-than -d 'Maximum size of files to be backed up. Larger files will be excluded' -r
complete -c rustic -n "__fish_rustic_using_subcommand diff" -s P -l use-profile -d 'Config profile to use. This parses the file `<PROFILE>.toml` in the config directory. [default: "rustic"]' -r
complete -c rustic -n "__fish_rustic_using_subcommand diff" -l log-level -d 'Use this log level [default: info]' -r
complete -c rustic -n "__fish_rustic_using_subcommand diff" -l log-file -d 'Write log messages to the given file instead of printing them' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand diff" -l progress-interval -d 'Interval to update progress bars' -r
complete -c rustic -n "__fish_rustic_using_subcommand diff" -s r -l repository -l repo -d 'Repository to use' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand diff" -l repo-hot -d 'Repository to use as hot storage' -r
complete -c rustic -n "__fish_rustic_using_subcommand diff" -l password -d 'Password of the repository' -r
complete -c rustic -n "__fish_rustic_using_subcommand diff" -s p -l password-file -d 'File to read the password from' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand diff" -l password-command -d 'Command to read the password from. Password is read from stdout' -r
complete -c rustic -n "__fish_rustic_using_subcommand diff" -l cache-dir -d 'Use this dir as cache dir instead of the standard cache dir' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand diff" -l warm-up-command -d 'Warm up needed data pack files by running the command with %id replaced by pack id' -r
complete -c rustic -n "__fish_rustic_using_subcommand diff" -l warm-up-wait -d 'Duration (e.g. 10m) to wait after warm up' -r
complete -c rustic -n "__fish_rustic_using_subcommand diff" -l filter-host -d 'Hostname to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand diff" -l filter-label -d 'Label to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand diff" -l filter-paths -d 'Path list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand diff" -l filter-tags -d 'Tag list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand diff" -l filter-fn -d 'Function to filter snapshots' -r
complete -c rustic -n "__fish_rustic_using_subcommand diff" -l metadata -d 'show differences in metadata'
complete -c rustic -n "__fish_rustic_using_subcommand diff" -l no-content -d 'don\'t check for different file contents'
complete -c rustic -n "__fish_rustic_using_subcommand diff" -l git-ignore -d 'Ignore files based on .gitignore files'
complete -c rustic -n "__fish_rustic_using_subcommand diff" -l no-require-git -d 'Do not require a git repository to apply git-ignore rule'
complete -c rustic -n "__fish_rustic_using_subcommand diff" -s x -l one-file-system -d 'Exclude other file systems, don\'t cross filesystem boundaries and subvolumes'
complete -c rustic -n "__fish_rustic_using_subcommand diff" -s n -l dry-run -d 'Only show what would be done without modifying anything. Does not affect read-only commands'
complete -c rustic -n "__fish_rustic_using_subcommand diff" -l check-index -d 'Check if index matches pack files and read pack headers if neccessary'
complete -c rustic -n "__fish_rustic_using_subcommand diff" -l no-progress -d 'Don\'t show any progress bar'
complete -c rustic -n "__fish_rustic_using_subcommand diff" -l no-cache -d 'Don\'t use a cache'
complete -c rustic -n "__fish_rustic_using_subcommand diff" -l warm-up -d 'Warm up needed data pack files by only requesting them without processing'
complete -c rustic -n "__fish_rustic_using_subcommand diff" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c rustic -n "__fish_rustic_using_subcommand dump" -s P -l use-profile -d 'Config profile to use. This parses the file `<PROFILE>.toml` in the config directory. [default: "rustic"]' -r
complete -c rustic -n "__fish_rustic_using_subcommand dump" -l log-level -d 'Use this log level [default: info]' -r
complete -c rustic -n "__fish_rustic_using_subcommand dump" -l log-file -d 'Write log messages to the given file instead of printing them' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand dump" -l progress-interval -d 'Interval to update progress bars' -r
complete -c rustic -n "__fish_rustic_using_subcommand dump" -s r -l repository -l repo -d 'Repository to use' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand dump" -l repo-hot -d 'Repository to use as hot storage' -r
complete -c rustic -n "__fish_rustic_using_subcommand dump" -l password -d 'Password of the repository' -r
complete -c rustic -n "__fish_rustic_using_subcommand dump" -s p -l password-file -d 'File to read the password from' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand dump" -l password-command -d 'Command to read the password from. Password is read from stdout' -r
complete -c rustic -n "__fish_rustic_using_subcommand dump" -l cache-dir -d 'Use this dir as cache dir instead of the standard cache dir' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand dump" -l warm-up-command -d 'Warm up needed data pack files by running the command with %id replaced by pack id' -r
complete -c rustic -n "__fish_rustic_using_subcommand dump" -l warm-up-wait -d 'Duration (e.g. 10m) to wait after warm up' -r
complete -c rustic -n "__fish_rustic_using_subcommand dump" -l filter-host -d 'Hostname to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand dump" -l filter-label -d 'Label to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand dump" -l filter-paths -d 'Path list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand dump" -l filter-tags -d 'Tag list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand dump" -l filter-fn -d 'Function to filter snapshots' -r
complete -c rustic -n "__fish_rustic_using_subcommand dump" -s n -l dry-run -d 'Only show what would be done without modifying anything. Does not affect read-only commands'
complete -c rustic -n "__fish_rustic_using_subcommand dump" -l check-index -d 'Check if index matches pack files and read pack headers if neccessary'
complete -c rustic -n "__fish_rustic_using_subcommand dump" -l no-progress -d 'Don\'t show any progress bar'
complete -c rustic -n "__fish_rustic_using_subcommand dump" -l no-cache -d 'Don\'t use a cache'
complete -c rustic -n "__fish_rustic_using_subcommand dump" -l warm-up -d 'Warm up needed data pack files by only requesting them without processing'
complete -c rustic -n "__fish_rustic_using_subcommand dump" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c rustic -n "__fish_rustic_using_subcommand find" -l glob -d 'pattern to find (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand find" -l iglob -d 'pattern to find case-insensitive (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand find" -l path -d 'exact path to find' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand find" -s g -l group-by -d 'Group snapshots by any combination of host,label,paths,tags' -r
complete -c rustic -n "__fish_rustic_using_subcommand find" -s P -l use-profile -d 'Config profile to use. This parses the file `<PROFILE>.toml` in the config directory. [default: "rustic"]' -r
complete -c rustic -n "__fish_rustic_using_subcommand find" -l log-level -d 'Use this log level [default: info]' -r
complete -c rustic -n "__fish_rustic_using_subcommand find" -l log-file -d 'Write log messages to the given file instead of printing them' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand find" -l progress-interval -d 'Interval to update progress bars' -r
complete -c rustic -n "__fish_rustic_using_subcommand find" -s r -l repository -l repo -d 'Repository to use' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand find" -l repo-hot -d 'Repository to use as hot storage' -r
complete -c rustic -n "__fish_rustic_using_subcommand find" -l password -d 'Password of the repository' -r
complete -c rustic -n "__fish_rustic_using_subcommand find" -s p -l password-file -d 'File to read the password from' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand find" -l password-command -d 'Command to read the password from. Password is read from stdout' -r
complete -c rustic -n "__fish_rustic_using_subcommand find" -l cache-dir -d 'Use this dir as cache dir instead of the standard cache dir' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand find" -l warm-up-command -d 'Warm up needed data pack files by running the command with %id replaced by pack id' -r
complete -c rustic -n "__fish_rustic_using_subcommand find" -l warm-up-wait -d 'Duration (e.g. 10m) to wait after warm up' -r
complete -c rustic -n "__fish_rustic_using_subcommand find" -l filter-host -d 'Hostname to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand find" -l filter-label -d 'Label to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand find" -l filter-paths -d 'Path list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand find" -l filter-tags -d 'Tag list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand find" -l filter-fn -d 'Function to filter snapshots' -r
complete -c rustic -n "__fish_rustic_using_subcommand find" -l all -d 'Show all snapshots instead of summarizing snapshots with identical search results'
complete -c rustic -n "__fish_rustic_using_subcommand find" -l show-misses -d 'Also show snapshots which don\'t contain a search result'
complete -c rustic -n "__fish_rustic_using_subcommand find" -l numeric-uid-gid -d 'Show uid/gid instead of user/group'
complete -c rustic -n "__fish_rustic_using_subcommand find" -s n -l dry-run -d 'Only show what would be done without modifying anything. Does not affect read-only commands'
complete -c rustic -n "__fish_rustic_using_subcommand find" -l check-index -d 'Check if index matches pack files and read pack headers if neccessary'
complete -c rustic -n "__fish_rustic_using_subcommand find" -l no-progress -d 'Don\'t show any progress bar'
complete -c rustic -n "__fish_rustic_using_subcommand find" -l no-cache -d 'Don\'t use a cache'
complete -c rustic -n "__fish_rustic_using_subcommand find" -l warm-up -d 'Warm up needed data pack files by only requesting them without processing'
complete -c rustic -n "__fish_rustic_using_subcommand find" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c rustic -n "__fish_rustic_using_subcommand forget" -s g -l group-by -d 'Group snapshots by any combination of host,label,paths,tags (default: "host,label,paths")' -r
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l filter-host -d 'Hostname to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l filter-label -d 'Label to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l filter-paths -d 'Path list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l filter-tags -d 'Tag list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l filter-fn -d 'Function to filter snapshots' -r
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l keep-tags -d 'Keep snapshots with this taglist (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l keep-id -d 'Keep snapshots ids that start with ID (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand forget" -s l -l keep-last -d 'Keep the last N snapshots (N == -1: keep all snapshots)' -r
complete -c rustic -n "__fish_rustic_using_subcommand forget" -s H -l keep-hourly -d 'Keep the last N hourly snapshots (N == -1: keep all hourly snapshots)' -r
complete -c rustic -n "__fish_rustic_using_subcommand forget" -s d -l keep-daily -d 'Keep the last N daily snapshots (N == -1: keep all daily snapshots)' -r
complete -c rustic -n "__fish_rustic_using_subcommand forget" -s w -l keep-weekly -d 'Keep the last N weekly snapshots (N == -1: keep all weekly snapshots)' -r
complete -c rustic -n "__fish_rustic_using_subcommand forget" -s m -l keep-monthly -d 'Keep the last N monthly snapshots (N == -1: keep all monthly snapshots)' -r
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l keep-quarter-yearly -d 'Keep the last N quarter-yearly snapshots (N == -1: keep all quarter-yearly snapshots)' -r
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l keep-half-yearly -d 'Keep the last N half-yearly snapshots (N == -1: keep all half-yearly snapshots)' -r
complete -c rustic -n "__fish_rustic_using_subcommand forget" -s y -l keep-yearly -d 'Keep the last N yearly snapshots (N == -1: keep all yearly snapshots)' -r
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l keep-within -d 'Keep snapshots newer than DURATION relative to latest snapshot' -r
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l keep-within-hourly -d 'Keep hourly snapshots newer than DURATION relative to latest snapshot' -r
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l keep-within-daily -d 'Keep daily snapshots newer than DURATION relative to latest snapshot' -r
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l keep-within-weekly -d 'Keep weekly snapshots newer than DURATION relative to latest snapshot' -r
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l keep-within-monthly -d 'Keep monthly snapshots newer than DURATION relative to latest snapshot' -r
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l keep-within-quarter-yearly -d 'Keep quarter-yearly snapshots newer than DURATION relative to latest snapshot' -r
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l keep-within-half-yearly -d 'Keep half-yearly snapshots newer than DURATION relative to latest snapshot' -r
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l keep-within-yearly -d 'Keep yearly snapshots newer than DURATION relative to latest snapshot' -r
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l max-repack -d 'Define maximum data to repack in % of reposize or as size (e.g. \'5b\', \'2 kB\', \'3M\', \'4TiB\') or \'unlimited\'' -r
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l max-unused -d 'Tolerate limit of unused data in % of reposize after pruning or as size (e.g. \'5b\', \'2 kB\', \'3M\', \'4TiB\') or \'unlimited\'' -r
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l keep-pack -d 'Minimum duration (e.g. 90d) to keep packs before repacking or removing. More recently created packs won\'t be repacked or marked for deletion within this prune run' -r
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l keep-delete -d 'Minimum duration (e.g. 10m) to keep packs marked for deletion. More recently marked packs won\'t be deleted within this prune run' -r
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l repack-cacheable-only -d 'Only repack packs which are cacheable [default: true for a hot/cold repository, else false]' -r -f -a "{true\t'',false\t''}"
complete -c rustic -n "__fish_rustic_using_subcommand forget" -s P -l use-profile -d 'Config profile to use. This parses the file `<PROFILE>.toml` in the config directory. [default: "rustic"]' -r
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l log-level -d 'Use this log level [default: info]' -r
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l log-file -d 'Write log messages to the given file instead of printing them' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l progress-interval -d 'Interval to update progress bars' -r
complete -c rustic -n "__fish_rustic_using_subcommand forget" -s r -l repository -l repo -d 'Repository to use' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l repo-hot -d 'Repository to use as hot storage' -r
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l password -d 'Password of the repository' -r
complete -c rustic -n "__fish_rustic_using_subcommand forget" -s p -l password-file -d 'File to read the password from' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l password-command -d 'Command to read the password from. Password is read from stdout' -r
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l cache-dir -d 'Use this dir as cache dir instead of the standard cache dir' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l warm-up-command -d 'Warm up needed data pack files by running the command with %id replaced by pack id' -r
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l warm-up-wait -d 'Duration (e.g. 10m) to wait after warm up' -r
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l json -d 'Show infos in json format'
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l quiet -d 'Don\'t show any output'
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l prune -d 'Also prune the repository'
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l keep-none -d 'Allow to keep no snapshot'
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l instant-delete -d 'Delete files immediately instead of marking them. This also removes all files already marked for deletion'
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l early-delete-index -d 'Delete index files early. This allows to run prune if there is few or no space left'
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l fast-repack -d 'Simply copy blobs when repacking instead of decrypting; possibly compressing; encrypting'
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l repack-uncompressed -d 'Repack packs containing uncompressed blobs. This cannot be used with --fast-repack. Implies --max-unused=0'
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l repack-all -d 'Repack all packs. Implies --max-unused=0'
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l no-resize -d 'Do not repack packs which only needs to be resized'
complete -c rustic -n "__fish_rustic_using_subcommand forget" -s n -l dry-run -d 'Only show what would be done without modifying anything. Does not affect read-only commands'
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l check-index -d 'Check if index matches pack files and read pack headers if neccessary'
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l no-progress -d 'Don\'t show any progress bar'
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l no-cache -d 'Don\'t use a cache'
complete -c rustic -n "__fish_rustic_using_subcommand forget" -l warm-up -d 'Warm up needed data pack files by only requesting them without processing'
complete -c rustic -n "__fish_rustic_using_subcommand forget" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c rustic -n "__fish_rustic_using_subcommand init" -l hostname -d 'Set \'hostname\' in public key information' -r
complete -c rustic -n "__fish_rustic_using_subcommand init" -l username -d 'Set \'username\' in public key information' -r
complete -c rustic -n "__fish_rustic_using_subcommand init" -l set-compression -d 'Set compression level. Allowed levels are 1 to 22 and -1 to -7, see <https://facebook.github.io/zstd/>. Note that 0 equals to no compression' -r
complete -c rustic -n "__fish_rustic_using_subcommand init" -l set-version -d 'Set repository version. Allowed versions: 1,2' -r
complete -c rustic -n "__fish_rustic_using_subcommand init" -l set-append-only -d 'Set append-only mode. Note that only append-only commands work once this is set. `forget`, `prune` or `config` won\'t work any longer' -r -f -a "{true\t'',false\t''}"
complete -c rustic -n "__fish_rustic_using_subcommand init" -l set-treepack-size -d 'Set default packsize for tree packs. rustic tries to always produce packs greater than this value. Note that for large repos, this value is grown by the grown factor. Defaults to `4 MiB` if not set' -r
complete -c rustic -n "__fish_rustic_using_subcommand init" -l set-treepack-size-limit -d 'Set upper limit for default packsize for tree packs. Note that packs actually can get a bit larger. If not set, pack sizes can grow up to approximately `4 GiB`' -r
complete -c rustic -n "__fish_rustic_using_subcommand init" -l set-treepack-growfactor -d 'Set grow factor for tree packs. The default packsize grows by the square root of the total size of all tree packs multiplied with this factor. This means 32 kiB times this factor per square root of total treesize in GiB. Defaults to `32` (= 1MB per square root of total treesize in GiB) if not set' -r
complete -c rustic -n "__fish_rustic_using_subcommand init" -l set-datapack-size -d 'Set default packsize for data packs. rustic tries to always produce packs greater than this value. Note that for large repos, this value is grown by the grown factor. Defaults to `32 MiB` if not set' -r
complete -c rustic -n "__fish_rustic_using_subcommand init" -l set-datapack-growfactor -d 'Set grow factor for data packs. The default packsize grows by the square root of the total size of all data packs multiplied with this factor. This means 32 kiB times this factor per square root of total datasize in GiB. Defaults to `32` (= 1MB per square root of total datasize in GiB) if not set' -r
complete -c rustic -n "__fish_rustic_using_subcommand init" -l set-datapack-size-limit -d 'Set upper limit for default packsize for tree packs. Note that packs actually can get a bit larger. If not set, pack sizes can grow up to approximately `4 GiB`' -r
complete -c rustic -n "__fish_rustic_using_subcommand init" -l set-min-packsize-tolerate-percent -d 'Set minimum tolerated packsize in percent of the targeted packsize. Defaults to `30` if not set' -r
complete -c rustic -n "__fish_rustic_using_subcommand init" -l set-max-packsize-tolerate-percent -d 'Set maximum tolerated packsize in percent of the targeted packsize A value of `0` means packs larger than the targeted packsize are always tolerated. Default if not set: larger packfiles are always tolerated' -r
complete -c rustic -n "__fish_rustic_using_subcommand init" -l set-extra-verify -d 'Do an extra verification by decompressing/decrypting all data before uploading to the repository. Default: true' -r -f -a "{true\t'',false\t''}"
complete -c rustic -n "__fish_rustic_using_subcommand init" -s P -l use-profile -d 'Config profile to use. This parses the file `<PROFILE>.toml` in the config directory. [default: "rustic"]' -r
complete -c rustic -n "__fish_rustic_using_subcommand init" -l log-level -d 'Use this log level [default: info]' -r
complete -c rustic -n "__fish_rustic_using_subcommand init" -l log-file -d 'Write log messages to the given file instead of printing them' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand init" -l progress-interval -d 'Interval to update progress bars' -r
complete -c rustic -n "__fish_rustic_using_subcommand init" -s r -l repository -l repo -d 'Repository to use' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand init" -l repo-hot -d 'Repository to use as hot storage' -r
complete -c rustic -n "__fish_rustic_using_subcommand init" -l password -d 'Password of the repository' -r
complete -c rustic -n "__fish_rustic_using_subcommand init" -s p -l password-file -d 'File to read the password from' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand init" -l password-command -d 'Command to read the password from. Password is read from stdout' -r
complete -c rustic -n "__fish_rustic_using_subcommand init" -l cache-dir -d 'Use this dir as cache dir instead of the standard cache dir' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand init" -l warm-up-command -d 'Warm up needed data pack files by running the command with %id replaced by pack id' -r
complete -c rustic -n "__fish_rustic_using_subcommand init" -l warm-up-wait -d 'Duration (e.g. 10m) to wait after warm up' -r
complete -c rustic -n "__fish_rustic_using_subcommand init" -l filter-host -d 'Hostname to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand init" -l filter-label -d 'Label to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand init" -l filter-paths -d 'Path list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand init" -l filter-tags -d 'Tag list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand init" -l filter-fn -d 'Function to filter snapshots' -r
complete -c rustic -n "__fish_rustic_using_subcommand init" -l with-created -d 'Add \'created\' date in public key information'
complete -c rustic -n "__fish_rustic_using_subcommand init" -s n -l dry-run -d 'Only show what would be done without modifying anything. Does not affect read-only commands'
complete -c rustic -n "__fish_rustic_using_subcommand init" -l check-index -d 'Check if index matches pack files and read pack headers if neccessary'
complete -c rustic -n "__fish_rustic_using_subcommand init" -l no-progress -d 'Don\'t show any progress bar'
complete -c rustic -n "__fish_rustic_using_subcommand init" -l no-cache -d 'Don\'t use a cache'
complete -c rustic -n "__fish_rustic_using_subcommand init" -l warm-up -d 'Warm up needed data pack files by only requesting them without processing'
complete -c rustic -n "__fish_rustic_using_subcommand init" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c rustic -n "__fish_rustic_using_subcommand key; and not __fish_seen_subcommand_from add help" -s P -l use-profile -d 'Config profile to use. This parses the file `<PROFILE>.toml` in the config directory. [default: "rustic"]' -r
complete -c rustic -n "__fish_rustic_using_subcommand key; and not __fish_seen_subcommand_from add help" -l log-level -d 'Use this log level [default: info]' -r
complete -c rustic -n "__fish_rustic_using_subcommand key; and not __fish_seen_subcommand_from add help" -l log-file -d 'Write log messages to the given file instead of printing them' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand key; and not __fish_seen_subcommand_from add help" -l progress-interval -d 'Interval to update progress bars' -r
complete -c rustic -n "__fish_rustic_using_subcommand key; and not __fish_seen_subcommand_from add help" -s r -l repository -l repo -d 'Repository to use' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand key; and not __fish_seen_subcommand_from add help" -l repo-hot -d 'Repository to use as hot storage' -r
complete -c rustic -n "__fish_rustic_using_subcommand key; and not __fish_seen_subcommand_from add help" -l password -d 'Password of the repository' -r
complete -c rustic -n "__fish_rustic_using_subcommand key; and not __fish_seen_subcommand_from add help" -s p -l password-file -d 'File to read the password from' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand key; and not __fish_seen_subcommand_from add help" -l password-command -d 'Command to read the password from. Password is read from stdout' -r
complete -c rustic -n "__fish_rustic_using_subcommand key; and not __fish_seen_subcommand_from add help" -l cache-dir -d 'Use this dir as cache dir instead of the standard cache dir' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand key; and not __fish_seen_subcommand_from add help" -l warm-up-command -d 'Warm up needed data pack files by running the command with %id replaced by pack id' -r
complete -c rustic -n "__fish_rustic_using_subcommand key; and not __fish_seen_subcommand_from add help" -l warm-up-wait -d 'Duration (e.g. 10m) to wait after warm up' -r
complete -c rustic -n "__fish_rustic_using_subcommand key; and not __fish_seen_subcommand_from add help" -l filter-host -d 'Hostname to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand key; and not __fish_seen_subcommand_from add help" -l filter-label -d 'Label to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand key; and not __fish_seen_subcommand_from add help" -l filter-paths -d 'Path list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand key; and not __fish_seen_subcommand_from add help" -l filter-tags -d 'Tag list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand key; and not __fish_seen_subcommand_from add help" -l filter-fn -d 'Function to filter snapshots' -r
complete -c rustic -n "__fish_rustic_using_subcommand key; and not __fish_seen_subcommand_from add help" -s n -l dry-run -d 'Only show what would be done without modifying anything. Does not affect read-only commands'
complete -c rustic -n "__fish_rustic_using_subcommand key; and not __fish_seen_subcommand_from add help" -l check-index -d 'Check if index matches pack files and read pack headers if neccessary'
complete -c rustic -n "__fish_rustic_using_subcommand key; and not __fish_seen_subcommand_from add help" -l no-progress -d 'Don\'t show any progress bar'
complete -c rustic -n "__fish_rustic_using_subcommand key; and not __fish_seen_subcommand_from add help" -l no-cache -d 'Don\'t use a cache'
complete -c rustic -n "__fish_rustic_using_subcommand key; and not __fish_seen_subcommand_from add help" -l warm-up -d 'Warm up needed data pack files by only requesting them without processing'
complete -c rustic -n "__fish_rustic_using_subcommand key; and not __fish_seen_subcommand_from add help" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c rustic -n "__fish_rustic_using_subcommand key; and not __fish_seen_subcommand_from add help" -f -a "add" -d 'Add a new key to the repository'
complete -c rustic -n "__fish_rustic_using_subcommand key; and not __fish_seen_subcommand_from add help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rustic -n "__fish_rustic_using_subcommand key; and __fish_seen_subcommand_from add" -l new-password -d 'New password' -r
complete -c rustic -n "__fish_rustic_using_subcommand key; and __fish_seen_subcommand_from add" -l new-password-file -d 'File from which to read the new password' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand key; and __fish_seen_subcommand_from add" -l new-password-command -d 'Command to get the new password from' -r
complete -c rustic -n "__fish_rustic_using_subcommand key; and __fish_seen_subcommand_from add" -l hostname -d 'Set \'hostname\' in public key information' -r
complete -c rustic -n "__fish_rustic_using_subcommand key; and __fish_seen_subcommand_from add" -l username -d 'Set \'username\' in public key information' -r
complete -c rustic -n "__fish_rustic_using_subcommand key; and __fish_seen_subcommand_from add" -s P -l use-profile -d 'Config profile to use. This parses the file `<PROFILE>.toml` in the config directory. [default: "rustic"]' -r
complete -c rustic -n "__fish_rustic_using_subcommand key; and __fish_seen_subcommand_from add" -l log-level -d 'Use this log level [default: info]' -r
complete -c rustic -n "__fish_rustic_using_subcommand key; and __fish_seen_subcommand_from add" -l log-file -d 'Write log messages to the given file instead of printing them' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand key; and __fish_seen_subcommand_from add" -l progress-interval -d 'Interval to update progress bars' -r
complete -c rustic -n "__fish_rustic_using_subcommand key; and __fish_seen_subcommand_from add" -s r -l repository -l repo -d 'Repository to use' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand key; and __fish_seen_subcommand_from add" -l repo-hot -d 'Repository to use as hot storage' -r
complete -c rustic -n "__fish_rustic_using_subcommand key; and __fish_seen_subcommand_from add" -l password -d 'Password of the repository' -r
complete -c rustic -n "__fish_rustic_using_subcommand key; and __fish_seen_subcommand_from add" -s p -l password-file -d 'File to read the password from' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand key; and __fish_seen_subcommand_from add" -l password-command -d 'Command to read the password from. Password is read from stdout' -r
complete -c rustic -n "__fish_rustic_using_subcommand key; and __fish_seen_subcommand_from add" -l cache-dir -d 'Use this dir as cache dir instead of the standard cache dir' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand key; and __fish_seen_subcommand_from add" -l warm-up-command -d 'Warm up needed data pack files by running the command with %id replaced by pack id' -r
complete -c rustic -n "__fish_rustic_using_subcommand key; and __fish_seen_subcommand_from add" -l warm-up-wait -d 'Duration (e.g. 10m) to wait after warm up' -r
complete -c rustic -n "__fish_rustic_using_subcommand key; and __fish_seen_subcommand_from add" -l filter-host -d 'Hostname to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand key; and __fish_seen_subcommand_from add" -l filter-label -d 'Label to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand key; and __fish_seen_subcommand_from add" -l filter-paths -d 'Path list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand key; and __fish_seen_subcommand_from add" -l filter-tags -d 'Tag list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand key; and __fish_seen_subcommand_from add" -l filter-fn -d 'Function to filter snapshots' -r
complete -c rustic -n "__fish_rustic_using_subcommand key; and __fish_seen_subcommand_from add" -l with-created -d 'Add \'created\' date in public key information'
complete -c rustic -n "__fish_rustic_using_subcommand key; and __fish_seen_subcommand_from add" -s n -l dry-run -d 'Only show what would be done without modifying anything. Does not affect read-only commands'
complete -c rustic -n "__fish_rustic_using_subcommand key; and __fish_seen_subcommand_from add" -l check-index -d 'Check if index matches pack files and read pack headers if neccessary'
complete -c rustic -n "__fish_rustic_using_subcommand key; and __fish_seen_subcommand_from add" -l no-progress -d 'Don\'t show any progress bar'
complete -c rustic -n "__fish_rustic_using_subcommand key; and __fish_seen_subcommand_from add" -l no-cache -d 'Don\'t use a cache'
complete -c rustic -n "__fish_rustic_using_subcommand key; and __fish_seen_subcommand_from add" -l warm-up -d 'Warm up needed data pack files by only requesting them without processing'
complete -c rustic -n "__fish_rustic_using_subcommand key; and __fish_seen_subcommand_from add" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c rustic -n "__fish_rustic_using_subcommand key; and __fish_seen_subcommand_from help" -f -a "add" -d 'Add a new key to the repository'
complete -c rustic -n "__fish_rustic_using_subcommand key; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rustic -n "__fish_rustic_using_subcommand list" -s P -l use-profile -d 'Config profile to use. This parses the file `<PROFILE>.toml` in the config directory. [default: "rustic"]' -r
complete -c rustic -n "__fish_rustic_using_subcommand list" -l log-level -d 'Use this log level [default: info]' -r
complete -c rustic -n "__fish_rustic_using_subcommand list" -l log-file -d 'Write log messages to the given file instead of printing them' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand list" -l progress-interval -d 'Interval to update progress bars' -r
complete -c rustic -n "__fish_rustic_using_subcommand list" -s r -l repository -l repo -d 'Repository to use' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand list" -l repo-hot -d 'Repository to use as hot storage' -r
complete -c rustic -n "__fish_rustic_using_subcommand list" -l password -d 'Password of the repository' -r
complete -c rustic -n "__fish_rustic_using_subcommand list" -s p -l password-file -d 'File to read the password from' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand list" -l password-command -d 'Command to read the password from. Password is read from stdout' -r
complete -c rustic -n "__fish_rustic_using_subcommand list" -l cache-dir -d 'Use this dir as cache dir instead of the standard cache dir' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand list" -l warm-up-command -d 'Warm up needed data pack files by running the command with %id replaced by pack id' -r
complete -c rustic -n "__fish_rustic_using_subcommand list" -l warm-up-wait -d 'Duration (e.g. 10m) to wait after warm up' -r
complete -c rustic -n "__fish_rustic_using_subcommand list" -l filter-host -d 'Hostname to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand list" -l filter-label -d 'Label to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand list" -l filter-paths -d 'Path list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand list" -l filter-tags -d 'Tag list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand list" -l filter-fn -d 'Function to filter snapshots' -r
complete -c rustic -n "__fish_rustic_using_subcommand list" -s n -l dry-run -d 'Only show what would be done without modifying anything. Does not affect read-only commands'
complete -c rustic -n "__fish_rustic_using_subcommand list" -l check-index -d 'Check if index matches pack files and read pack headers if neccessary'
complete -c rustic -n "__fish_rustic_using_subcommand list" -l no-progress -d 'Don\'t show any progress bar'
complete -c rustic -n "__fish_rustic_using_subcommand list" -l no-cache -d 'Don\'t use a cache'
complete -c rustic -n "__fish_rustic_using_subcommand list" -l warm-up -d 'Warm up needed data pack files by only requesting them without processing'
complete -c rustic -n "__fish_rustic_using_subcommand list" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c rustic -n "__fish_rustic_using_subcommand ls" -l glob -d 'Glob pattern to exclude/include (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand ls" -l iglob -d 'Same as --glob pattern but ignores the casing of filenames' -r
complete -c rustic -n "__fish_rustic_using_subcommand ls" -l glob-file -d 'Read glob patterns to exclude/include from this file (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand ls" -l iglob-file -d 'Same as --glob-file ignores the casing of filenames in patterns' -r
complete -c rustic -n "__fish_rustic_using_subcommand ls" -s P -l use-profile -d 'Config profile to use. This parses the file `<PROFILE>.toml` in the config directory. [default: "rustic"]' -r
complete -c rustic -n "__fish_rustic_using_subcommand ls" -l log-level -d 'Use this log level [default: info]' -r
complete -c rustic -n "__fish_rustic_using_subcommand ls" -l log-file -d 'Write log messages to the given file instead of printing them' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand ls" -l progress-interval -d 'Interval to update progress bars' -r
complete -c rustic -n "__fish_rustic_using_subcommand ls" -s r -l repository -l repo -d 'Repository to use' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand ls" -l repo-hot -d 'Repository to use as hot storage' -r
complete -c rustic -n "__fish_rustic_using_subcommand ls" -l password -d 'Password of the repository' -r
complete -c rustic -n "__fish_rustic_using_subcommand ls" -s p -l password-file -d 'File to read the password from' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand ls" -l password-command -d 'Command to read the password from. Password is read from stdout' -r
complete -c rustic -n "__fish_rustic_using_subcommand ls" -l cache-dir -d 'Use this dir as cache dir instead of the standard cache dir' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand ls" -l warm-up-command -d 'Warm up needed data pack files by running the command with %id replaced by pack id' -r
complete -c rustic -n "__fish_rustic_using_subcommand ls" -l warm-up-wait -d 'Duration (e.g. 10m) to wait after warm up' -r
complete -c rustic -n "__fish_rustic_using_subcommand ls" -l filter-host -d 'Hostname to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand ls" -l filter-label -d 'Label to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand ls" -l filter-paths -d 'Path list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand ls" -l filter-tags -d 'Tag list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand ls" -l filter-fn -d 'Function to filter snapshots' -r
complete -c rustic -n "__fish_rustic_using_subcommand ls" -s s -l summary -d 'show summary'
complete -c rustic -n "__fish_rustic_using_subcommand ls" -s l -l long -d 'show long listing'
complete -c rustic -n "__fish_rustic_using_subcommand ls" -l numeric-uid-gid -d 'show uid/gid instead of user/group'
complete -c rustic -n "__fish_rustic_using_subcommand ls" -l recursive -d 'recursively list the dir'
complete -c rustic -n "__fish_rustic_using_subcommand ls" -s n -l dry-run -d 'Only show what would be done without modifying anything. Does not affect read-only commands'
complete -c rustic -n "__fish_rustic_using_subcommand ls" -l check-index -d 'Check if index matches pack files and read pack headers if neccessary'
complete -c rustic -n "__fish_rustic_using_subcommand ls" -l no-progress -d 'Don\'t show any progress bar'
complete -c rustic -n "__fish_rustic_using_subcommand ls" -l no-cache -d 'Don\'t use a cache'
complete -c rustic -n "__fish_rustic_using_subcommand ls" -l warm-up -d 'Warm up needed data pack files by only requesting them without processing'
complete -c rustic -n "__fish_rustic_using_subcommand ls" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c rustic -n "__fish_rustic_using_subcommand merge" -l label -d 'Label snapshot with given label' -r
complete -c rustic -n "__fish_rustic_using_subcommand merge" -l tag -d 'Tags to add to snapshot (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand merge" -l description -d 'Add description to snapshot' -r
complete -c rustic -n "__fish_rustic_using_subcommand merge" -l description-from -d 'Add description to snapshot from file' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand merge" -l time -d 'Set the backup time manually' -r
complete -c rustic -n "__fish_rustic_using_subcommand merge" -l delete-after -d 'Mark snapshot to be deleted after given duration (e.g. 10d)' -r
complete -c rustic -n "__fish_rustic_using_subcommand merge" -l host -d 'Set the host name manually' -r
complete -c rustic -n "__fish_rustic_using_subcommand merge" -l command -d 'Set the backup command manually' -r
complete -c rustic -n "__fish_rustic_using_subcommand merge" -s P -l use-profile -d 'Config profile to use. This parses the file `<PROFILE>.toml` in the config directory. [default: "rustic"]' -r
complete -c rustic -n "__fish_rustic_using_subcommand merge" -l log-level -d 'Use this log level [default: info]' -r
complete -c rustic -n "__fish_rustic_using_subcommand merge" -l log-file -d 'Write log messages to the given file instead of printing them' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand merge" -l progress-interval -d 'Interval to update progress bars' -r
complete -c rustic -n "__fish_rustic_using_subcommand merge" -s r -l repository -l repo -d 'Repository to use' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand merge" -l repo-hot -d 'Repository to use as hot storage' -r
complete -c rustic -n "__fish_rustic_using_subcommand merge" -l password -d 'Password of the repository' -r
complete -c rustic -n "__fish_rustic_using_subcommand merge" -s p -l password-file -d 'File to read the password from' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand merge" -l password-command -d 'Command to read the password from. Password is read from stdout' -r
complete -c rustic -n "__fish_rustic_using_subcommand merge" -l cache-dir -d 'Use this dir as cache dir instead of the standard cache dir' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand merge" -l warm-up-command -d 'Warm up needed data pack files by running the command with %id replaced by pack id' -r
complete -c rustic -n "__fish_rustic_using_subcommand merge" -l warm-up-wait -d 'Duration (e.g. 10m) to wait after warm up' -r
complete -c rustic -n "__fish_rustic_using_subcommand merge" -l filter-host -d 'Hostname to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand merge" -l filter-label -d 'Label to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand merge" -l filter-paths -d 'Path list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand merge" -l filter-tags -d 'Tag list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand merge" -l filter-fn -d 'Function to filter snapshots' -r
complete -c rustic -n "__fish_rustic_using_subcommand merge" -l json -d 'Output generated snapshot in json format'
complete -c rustic -n "__fish_rustic_using_subcommand merge" -l delete -d 'Remove input snapshots after merging'
complete -c rustic -n "__fish_rustic_using_subcommand merge" -l delete-never -d 'Mark snapshot as uneraseable'
complete -c rustic -n "__fish_rustic_using_subcommand merge" -s n -l dry-run -d 'Only show what would be done without modifying anything. Does not affect read-only commands'
complete -c rustic -n "__fish_rustic_using_subcommand merge" -l check-index -d 'Check if index matches pack files and read pack headers if neccessary'
complete -c rustic -n "__fish_rustic_using_subcommand merge" -l no-progress -d 'Don\'t show any progress bar'
complete -c rustic -n "__fish_rustic_using_subcommand merge" -l no-cache -d 'Don\'t use a cache'
complete -c rustic -n "__fish_rustic_using_subcommand merge" -l warm-up -d 'Warm up needed data pack files by only requesting them without processing'
complete -c rustic -n "__fish_rustic_using_subcommand merge" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c rustic -n "__fish_rustic_using_subcommand snapshots" -s g -l group-by -d 'Group snapshots by any combination of host,label,paths,tags' -r
complete -c rustic -n "__fish_rustic_using_subcommand snapshots" -s P -l use-profile -d 'Config profile to use. This parses the file `<PROFILE>.toml` in the config directory. [default: "rustic"]' -r
complete -c rustic -n "__fish_rustic_using_subcommand snapshots" -l log-level -d 'Use this log level [default: info]' -r
complete -c rustic -n "__fish_rustic_using_subcommand snapshots" -l log-file -d 'Write log messages to the given file instead of printing them' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand snapshots" -l progress-interval -d 'Interval to update progress bars' -r
complete -c rustic -n "__fish_rustic_using_subcommand snapshots" -s r -l repository -l repo -d 'Repository to use' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand snapshots" -l repo-hot -d 'Repository to use as hot storage' -r
complete -c rustic -n "__fish_rustic_using_subcommand snapshots" -l password -d 'Password of the repository' -r
complete -c rustic -n "__fish_rustic_using_subcommand snapshots" -s p -l password-file -d 'File to read the password from' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand snapshots" -l password-command -d 'Command to read the password from. Password is read from stdout' -r
complete -c rustic -n "__fish_rustic_using_subcommand snapshots" -l cache-dir -d 'Use this dir as cache dir instead of the standard cache dir' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand snapshots" -l warm-up-command -d 'Warm up needed data pack files by running the command with %id replaced by pack id' -r
complete -c rustic -n "__fish_rustic_using_subcommand snapshots" -l warm-up-wait -d 'Duration (e.g. 10m) to wait after warm up' -r
complete -c rustic -n "__fish_rustic_using_subcommand snapshots" -l filter-host -d 'Hostname to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand snapshots" -l filter-label -d 'Label to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand snapshots" -l filter-paths -d 'Path list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand snapshots" -l filter-tags -d 'Tag list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand snapshots" -l filter-fn -d 'Function to filter snapshots' -r
complete -c rustic -n "__fish_rustic_using_subcommand snapshots" -l long -d 'Show detailed information about snapshots'
complete -c rustic -n "__fish_rustic_using_subcommand snapshots" -l json -d 'Show snapshots in json format'
complete -c rustic -n "__fish_rustic_using_subcommand snapshots" -l all -d 'Show all snapshots instead of summarizing identical follow-up snapshots'
complete -c rustic -n "__fish_rustic_using_subcommand snapshots" -s i -l interactive -d 'Run in interactive UI mode'
complete -c rustic -n "__fish_rustic_using_subcommand snapshots" -s n -l dry-run -d 'Only show what would be done without modifying anything. Does not affect read-only commands'
complete -c rustic -n "__fish_rustic_using_subcommand snapshots" -l check-index -d 'Check if index matches pack files and read pack headers if neccessary'
complete -c rustic -n "__fish_rustic_using_subcommand snapshots" -l no-progress -d 'Don\'t show any progress bar'
complete -c rustic -n "__fish_rustic_using_subcommand snapshots" -l no-cache -d 'Don\'t use a cache'
complete -c rustic -n "__fish_rustic_using_subcommand snapshots" -l warm-up -d 'Warm up needed data pack files by only requesting them without processing'
complete -c rustic -n "__fish_rustic_using_subcommand snapshots" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c rustic -n "__fish_rustic_using_subcommand show-config" -s P -l use-profile -d 'Config profile to use. This parses the file `<PROFILE>.toml` in the config directory. [default: "rustic"]' -r
complete -c rustic -n "__fish_rustic_using_subcommand show-config" -l log-level -d 'Use this log level [default: info]' -r
complete -c rustic -n "__fish_rustic_using_subcommand show-config" -l log-file -d 'Write log messages to the given file instead of printing them' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand show-config" -l progress-interval -d 'Interval to update progress bars' -r
complete -c rustic -n "__fish_rustic_using_subcommand show-config" -s r -l repository -l repo -d 'Repository to use' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand show-config" -l repo-hot -d 'Repository to use as hot storage' -r
complete -c rustic -n "__fish_rustic_using_subcommand show-config" -l password -d 'Password of the repository' -r
complete -c rustic -n "__fish_rustic_using_subcommand show-config" -s p -l password-file -d 'File to read the password from' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand show-config" -l password-command -d 'Command to read the password from. Password is read from stdout' -r
complete -c rustic -n "__fish_rustic_using_subcommand show-config" -l cache-dir -d 'Use this dir as cache dir instead of the standard cache dir' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand show-config" -l warm-up-command -d 'Warm up needed data pack files by running the command with %id replaced by pack id' -r
complete -c rustic -n "__fish_rustic_using_subcommand show-config" -l warm-up-wait -d 'Duration (e.g. 10m) to wait after warm up' -r
complete -c rustic -n "__fish_rustic_using_subcommand show-config" -l filter-host -d 'Hostname to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand show-config" -l filter-label -d 'Label to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand show-config" -l filter-paths -d 'Path list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand show-config" -l filter-tags -d 'Tag list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand show-config" -l filter-fn -d 'Function to filter snapshots' -r
complete -c rustic -n "__fish_rustic_using_subcommand show-config" -s n -l dry-run -d 'Only show what would be done without modifying anything. Does not affect read-only commands'
complete -c rustic -n "__fish_rustic_using_subcommand show-config" -l check-index -d 'Check if index matches pack files and read pack headers if neccessary'
complete -c rustic -n "__fish_rustic_using_subcommand show-config" -l no-progress -d 'Don\'t show any progress bar'
complete -c rustic -n "__fish_rustic_using_subcommand show-config" -l no-cache -d 'Don\'t use a cache'
complete -c rustic -n "__fish_rustic_using_subcommand show-config" -l warm-up -d 'Warm up needed data pack files by only requesting them without processing'
complete -c rustic -n "__fish_rustic_using_subcommand show-config" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c rustic -n "__fish_rustic_using_subcommand self-update" -s P -l use-profile -d 'Config profile to use. This parses the file `<PROFILE>.toml` in the config directory. [default: "rustic"]' -r
complete -c rustic -n "__fish_rustic_using_subcommand self-update" -l log-level -d 'Use this log level [default: info]' -r
complete -c rustic -n "__fish_rustic_using_subcommand self-update" -l log-file -d 'Write log messages to the given file instead of printing them' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand self-update" -l progress-interval -d 'Interval to update progress bars' -r
complete -c rustic -n "__fish_rustic_using_subcommand self-update" -s r -l repository -l repo -d 'Repository to use' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand self-update" -l repo-hot -d 'Repository to use as hot storage' -r
complete -c rustic -n "__fish_rustic_using_subcommand self-update" -l password -d 'Password of the repository' -r
complete -c rustic -n "__fish_rustic_using_subcommand self-update" -s p -l password-file -d 'File to read the password from' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand self-update" -l password-command -d 'Command to read the password from. Password is read from stdout' -r
complete -c rustic -n "__fish_rustic_using_subcommand self-update" -l cache-dir -d 'Use this dir as cache dir instead of the standard cache dir' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand self-update" -l warm-up-command -d 'Warm up needed data pack files by running the command with %id replaced by pack id' -r
complete -c rustic -n "__fish_rustic_using_subcommand self-update" -l warm-up-wait -d 'Duration (e.g. 10m) to wait after warm up' -r
complete -c rustic -n "__fish_rustic_using_subcommand self-update" -l filter-host -d 'Hostname to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand self-update" -l filter-label -d 'Label to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand self-update" -l filter-paths -d 'Path list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand self-update" -l filter-tags -d 'Tag list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand self-update" -l filter-fn -d 'Function to filter snapshots' -r
complete -c rustic -n "__fish_rustic_using_subcommand self-update" -l force -d 'Do not ask before processing the self-update'
complete -c rustic -n "__fish_rustic_using_subcommand self-update" -s n -l dry-run -d 'Only show what would be done without modifying anything. Does not affect read-only commands'
complete -c rustic -n "__fish_rustic_using_subcommand self-update" -l check-index -d 'Check if index matches pack files and read pack headers if neccessary'
complete -c rustic -n "__fish_rustic_using_subcommand self-update" -l no-progress -d 'Don\'t show any progress bar'
complete -c rustic -n "__fish_rustic_using_subcommand self-update" -l no-cache -d 'Don\'t use a cache'
complete -c rustic -n "__fish_rustic_using_subcommand self-update" -l warm-up -d 'Warm up needed data pack files by only requesting them without processing'
complete -c rustic -n "__fish_rustic_using_subcommand self-update" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c rustic -n "__fish_rustic_using_subcommand prune" -l max-repack -d 'Define maximum data to repack in % of reposize or as size (e.g. \'5b\', \'2 kB\', \'3M\', \'4TiB\') or \'unlimited\'' -r
complete -c rustic -n "__fish_rustic_using_subcommand prune" -l max-unused -d 'Tolerate limit of unused data in % of reposize after pruning or as size (e.g. \'5b\', \'2 kB\', \'3M\', \'4TiB\') or \'unlimited\'' -r
complete -c rustic -n "__fish_rustic_using_subcommand prune" -l keep-pack -d 'Minimum duration (e.g. 90d) to keep packs before repacking or removing. More recently created packs won\'t be repacked or marked for deletion within this prune run' -r
complete -c rustic -n "__fish_rustic_using_subcommand prune" -l keep-delete -d 'Minimum duration (e.g. 10m) to keep packs marked for deletion. More recently marked packs won\'t be deleted within this prune run' -r
complete -c rustic -n "__fish_rustic_using_subcommand prune" -l repack-cacheable-only -d 'Only repack packs which are cacheable [default: true for a hot/cold repository, else false]' -r -f -a "{true\t'',false\t''}"
complete -c rustic -n "__fish_rustic_using_subcommand prune" -s P -l use-profile -d 'Config profile to use. This parses the file `<PROFILE>.toml` in the config directory. [default: "rustic"]' -r
complete -c rustic -n "__fish_rustic_using_subcommand prune" -l log-level -d 'Use this log level [default: info]' -r
complete -c rustic -n "__fish_rustic_using_subcommand prune" -l log-file -d 'Write log messages to the given file instead of printing them' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand prune" -l progress-interval -d 'Interval to update progress bars' -r
complete -c rustic -n "__fish_rustic_using_subcommand prune" -s r -l repository -l repo -d 'Repository to use' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand prune" -l repo-hot -d 'Repository to use as hot storage' -r
complete -c rustic -n "__fish_rustic_using_subcommand prune" -l password -d 'Password of the repository' -r
complete -c rustic -n "__fish_rustic_using_subcommand prune" -s p -l password-file -d 'File to read the password from' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand prune" -l password-command -d 'Command to read the password from. Password is read from stdout' -r
complete -c rustic -n "__fish_rustic_using_subcommand prune" -l cache-dir -d 'Use this dir as cache dir instead of the standard cache dir' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand prune" -l warm-up-command -d 'Warm up needed data pack files by running the command with %id replaced by pack id' -r
complete -c rustic -n "__fish_rustic_using_subcommand prune" -l warm-up-wait -d 'Duration (e.g. 10m) to wait after warm up' -r
complete -c rustic -n "__fish_rustic_using_subcommand prune" -l filter-host -d 'Hostname to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand prune" -l filter-label -d 'Label to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand prune" -l filter-paths -d 'Path list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand prune" -l filter-tags -d 'Tag list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand prune" -l filter-fn -d 'Function to filter snapshots' -r
complete -c rustic -n "__fish_rustic_using_subcommand prune" -l instant-delete -d 'Delete files immediately instead of marking them. This also removes all files already marked for deletion'
complete -c rustic -n "__fish_rustic_using_subcommand prune" -l early-delete-index -d 'Delete index files early. This allows to run prune if there is few or no space left'
complete -c rustic -n "__fish_rustic_using_subcommand prune" -l fast-repack -d 'Simply copy blobs when repacking instead of decrypting; possibly compressing; encrypting'
complete -c rustic -n "__fish_rustic_using_subcommand prune" -l repack-uncompressed -d 'Repack packs containing uncompressed blobs. This cannot be used with --fast-repack. Implies --max-unused=0'
complete -c rustic -n "__fish_rustic_using_subcommand prune" -l repack-all -d 'Repack all packs. Implies --max-unused=0'
complete -c rustic -n "__fish_rustic_using_subcommand prune" -l no-resize -d 'Do not repack packs which only needs to be resized'
complete -c rustic -n "__fish_rustic_using_subcommand prune" -s n -l dry-run -d 'Only show what would be done without modifying anything. Does not affect read-only commands'
complete -c rustic -n "__fish_rustic_using_subcommand prune" -l check-index -d 'Check if index matches pack files and read pack headers if neccessary'
complete -c rustic -n "__fish_rustic_using_subcommand prune" -l no-progress -d 'Don\'t show any progress bar'
complete -c rustic -n "__fish_rustic_using_subcommand prune" -l no-cache -d 'Don\'t use a cache'
complete -c rustic -n "__fish_rustic_using_subcommand prune" -l warm-up -d 'Warm up needed data pack files by only requesting them without processing'
complete -c rustic -n "__fish_rustic_using_subcommand prune" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c rustic -n "__fish_rustic_using_subcommand restore" -l glob -d 'Glob pattern to exclude/include (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand restore" -l iglob -d 'Same as --glob pattern but ignores the casing of filenames' -r
complete -c rustic -n "__fish_rustic_using_subcommand restore" -l glob-file -d 'Read glob patterns to exclude/include from this file (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand restore" -l iglob-file -d 'Same as --glob-file ignores the casing of filenames in patterns' -r
complete -c rustic -n "__fish_rustic_using_subcommand restore" -l filter-host -d 'Hostname to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand restore" -l filter-label -d 'Label to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand restore" -l filter-paths -d 'Path list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand restore" -l filter-tags -d 'Tag list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand restore" -l filter-fn -d 'Function to filter snapshots' -r
complete -c rustic -n "__fish_rustic_using_subcommand restore" -s P -l use-profile -d 'Config profile to use. This parses the file `<PROFILE>.toml` in the config directory. [default: "rustic"]' -r
complete -c rustic -n "__fish_rustic_using_subcommand restore" -l log-level -d 'Use this log level [default: info]' -r
complete -c rustic -n "__fish_rustic_using_subcommand restore" -l log-file -d 'Write log messages to the given file instead of printing them' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand restore" -l progress-interval -d 'Interval to update progress bars' -r
complete -c rustic -n "__fish_rustic_using_subcommand restore" -s r -l repository -l repo -d 'Repository to use' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand restore" -l repo-hot -d 'Repository to use as hot storage' -r
complete -c rustic -n "__fish_rustic_using_subcommand restore" -l password -d 'Password of the repository' -r
complete -c rustic -n "__fish_rustic_using_subcommand restore" -s p -l password-file -d 'File to read the password from' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand restore" -l password-command -d 'Command to read the password from. Password is read from stdout' -r
complete -c rustic -n "__fish_rustic_using_subcommand restore" -l cache-dir -d 'Use this dir as cache dir instead of the standard cache dir' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand restore" -l warm-up-command -d 'Warm up needed data pack files by running the command with %id replaced by pack id' -r
complete -c rustic -n "__fish_rustic_using_subcommand restore" -l warm-up-wait -d 'Duration (e.g. 10m) to wait after warm up' -r
complete -c rustic -n "__fish_rustic_using_subcommand restore" -l delete -d 'Remove all files/dirs in destination which are not contained in snapshot'
complete -c rustic -n "__fish_rustic_using_subcommand restore" -l numeric-id -d 'Use numeric ids instead of user/group when restoring uid/gui'
complete -c rustic -n "__fish_rustic_using_subcommand restore" -l no-ownership -d 'Don\'t restore ownership (user/group)'
complete -c rustic -n "__fish_rustic_using_subcommand restore" -l verify-existing -d 'Always read and verify existing files (don\'t trust correct modification time and file size)'
complete -c rustic -n "__fish_rustic_using_subcommand restore" -l recursive -d 'recursively list the dir'
complete -c rustic -n "__fish_rustic_using_subcommand restore" -s n -l dry-run -d 'Only show what would be done without modifying anything. Does not affect read-only commands'
complete -c rustic -n "__fish_rustic_using_subcommand restore" -l check-index -d 'Check if index matches pack files and read pack headers if neccessary'
complete -c rustic -n "__fish_rustic_using_subcommand restore" -l no-progress -d 'Don\'t show any progress bar'
complete -c rustic -n "__fish_rustic_using_subcommand restore" -l no-cache -d 'Don\'t use a cache'
complete -c rustic -n "__fish_rustic_using_subcommand restore" -l warm-up -d 'Warm up needed data pack files by only requesting them without processing'
complete -c rustic -n "__fish_rustic_using_subcommand restore" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c rustic -n "__fish_rustic_using_subcommand repair; and not __fish_seen_subcommand_from index snapshots help" -s P -l use-profile -d 'Config profile to use. This parses the file `<PROFILE>.toml` in the config directory. [default: "rustic"]' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and not __fish_seen_subcommand_from index snapshots help" -l log-level -d 'Use this log level [default: info]' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and not __fish_seen_subcommand_from index snapshots help" -l log-file -d 'Write log messages to the given file instead of printing them' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand repair; and not __fish_seen_subcommand_from index snapshots help" -l progress-interval -d 'Interval to update progress bars' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and not __fish_seen_subcommand_from index snapshots help" -s r -l repository -l repo -d 'Repository to use' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand repair; and not __fish_seen_subcommand_from index snapshots help" -l repo-hot -d 'Repository to use as hot storage' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and not __fish_seen_subcommand_from index snapshots help" -l password -d 'Password of the repository' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and not __fish_seen_subcommand_from index snapshots help" -s p -l password-file -d 'File to read the password from' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand repair; and not __fish_seen_subcommand_from index snapshots help" -l password-command -d 'Command to read the password from. Password is read from stdout' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and not __fish_seen_subcommand_from index snapshots help" -l cache-dir -d 'Use this dir as cache dir instead of the standard cache dir' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand repair; and not __fish_seen_subcommand_from index snapshots help" -l warm-up-command -d 'Warm up needed data pack files by running the command with %id replaced by pack id' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and not __fish_seen_subcommand_from index snapshots help" -l warm-up-wait -d 'Duration (e.g. 10m) to wait after warm up' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and not __fish_seen_subcommand_from index snapshots help" -l filter-host -d 'Hostname to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and not __fish_seen_subcommand_from index snapshots help" -l filter-label -d 'Label to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and not __fish_seen_subcommand_from index snapshots help" -l filter-paths -d 'Path list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and not __fish_seen_subcommand_from index snapshots help" -l filter-tags -d 'Tag list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and not __fish_seen_subcommand_from index snapshots help" -l filter-fn -d 'Function to filter snapshots' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and not __fish_seen_subcommand_from index snapshots help" -s n -l dry-run -d 'Only show what would be done without modifying anything. Does not affect read-only commands'
complete -c rustic -n "__fish_rustic_using_subcommand repair; and not __fish_seen_subcommand_from index snapshots help" -l check-index -d 'Check if index matches pack files and read pack headers if neccessary'
complete -c rustic -n "__fish_rustic_using_subcommand repair; and not __fish_seen_subcommand_from index snapshots help" -l no-progress -d 'Don\'t show any progress bar'
complete -c rustic -n "__fish_rustic_using_subcommand repair; and not __fish_seen_subcommand_from index snapshots help" -l no-cache -d 'Don\'t use a cache'
complete -c rustic -n "__fish_rustic_using_subcommand repair; and not __fish_seen_subcommand_from index snapshots help" -l warm-up -d 'Warm up needed data pack files by only requesting them without processing'
complete -c rustic -n "__fish_rustic_using_subcommand repair; and not __fish_seen_subcommand_from index snapshots help" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c rustic -n "__fish_rustic_using_subcommand repair; and not __fish_seen_subcommand_from index snapshots help" -f -a "index" -d 'Repair the repository index'
complete -c rustic -n "__fish_rustic_using_subcommand repair; and not __fish_seen_subcommand_from index snapshots help" -f -a "snapshots" -d 'Repair snapshots'
complete -c rustic -n "__fish_rustic_using_subcommand repair; and not __fish_seen_subcommand_from index snapshots help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from index" -s P -l use-profile -d 'Config profile to use. This parses the file `<PROFILE>.toml` in the config directory. [default: "rustic"]' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from index" -l log-level -d 'Use this log level [default: info]' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from index" -l log-file -d 'Write log messages to the given file instead of printing them' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from index" -l progress-interval -d 'Interval to update progress bars' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from index" -s r -l repository -l repo -d 'Repository to use' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from index" -l repo-hot -d 'Repository to use as hot storage' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from index" -l password -d 'Password of the repository' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from index" -s p -l password-file -d 'File to read the password from' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from index" -l password-command -d 'Command to read the password from. Password is read from stdout' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from index" -l cache-dir -d 'Use this dir as cache dir instead of the standard cache dir' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from index" -l warm-up-command -d 'Warm up needed data pack files by running the command with %id replaced by pack id' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from index" -l warm-up-wait -d 'Duration (e.g. 10m) to wait after warm up' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from index" -l filter-host -d 'Hostname to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from index" -l filter-label -d 'Label to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from index" -l filter-paths -d 'Path list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from index" -l filter-tags -d 'Tag list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from index" -l filter-fn -d 'Function to filter snapshots' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from index" -l read-all -d 'Read all data packs, i.e. completely re-create the index'
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from index" -s n -l dry-run -d 'Only show what would be done without modifying anything. Does not affect read-only commands'
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from index" -l check-index -d 'Check if index matches pack files and read pack headers if neccessary'
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from index" -l no-progress -d 'Don\'t show any progress bar'
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from index" -l no-cache -d 'Don\'t use a cache'
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from index" -l warm-up -d 'Warm up needed data pack files by only requesting them without processing'
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from index" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from snapshots" -l suffix -d 'Append this suffix to repaired directory or file name' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from snapshots" -l tag -d 'Tag list to set on repaired snapshots (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from snapshots" -s P -l use-profile -d 'Config profile to use. This parses the file `<PROFILE>.toml` in the config directory. [default: "rustic"]' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from snapshots" -l log-level -d 'Use this log level [default: info]' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from snapshots" -l log-file -d 'Write log messages to the given file instead of printing them' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from snapshots" -l progress-interval -d 'Interval to update progress bars' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from snapshots" -s r -l repository -l repo -d 'Repository to use' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from snapshots" -l repo-hot -d 'Repository to use as hot storage' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from snapshots" -l password -d 'Password of the repository' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from snapshots" -s p -l password-file -d 'File to read the password from' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from snapshots" -l password-command -d 'Command to read the password from. Password is read from stdout' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from snapshots" -l cache-dir -d 'Use this dir as cache dir instead of the standard cache dir' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from snapshots" -l warm-up-command -d 'Warm up needed data pack files by running the command with %id replaced by pack id' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from snapshots" -l warm-up-wait -d 'Duration (e.g. 10m) to wait after warm up' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from snapshots" -l filter-host -d 'Hostname to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from snapshots" -l filter-label -d 'Label to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from snapshots" -l filter-paths -d 'Path list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from snapshots" -l filter-tags -d 'Tag list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from snapshots" -l filter-fn -d 'Function to filter snapshots' -r
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from snapshots" -l delete -d 'Also remove defect snapshots'
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from snapshots" -s n -l dry-run -d 'Only show what would be done without modifying anything. Does not affect read-only commands'
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from snapshots" -l check-index -d 'Check if index matches pack files and read pack headers if neccessary'
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from snapshots" -l no-progress -d 'Don\'t show any progress bar'
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from snapshots" -l no-cache -d 'Don\'t use a cache'
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from snapshots" -l warm-up -d 'Warm up needed data pack files by only requesting them without processing'
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from snapshots" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from help" -f -a "index" -d 'Repair the repository index'
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from help" -f -a "snapshots" -d 'Repair snapshots'
complete -c rustic -n "__fish_rustic_using_subcommand repair; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rustic -n "__fish_rustic_using_subcommand repoinfo" -s P -l use-profile -d 'Config profile to use. This parses the file `<PROFILE>.toml` in the config directory. [default: "rustic"]' -r
complete -c rustic -n "__fish_rustic_using_subcommand repoinfo" -l log-level -d 'Use this log level [default: info]' -r
complete -c rustic -n "__fish_rustic_using_subcommand repoinfo" -l log-file -d 'Write log messages to the given file instead of printing them' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand repoinfo" -l progress-interval -d 'Interval to update progress bars' -r
complete -c rustic -n "__fish_rustic_using_subcommand repoinfo" -s r -l repository -l repo -d 'Repository to use' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand repoinfo" -l repo-hot -d 'Repository to use as hot storage' -r
complete -c rustic -n "__fish_rustic_using_subcommand repoinfo" -l password -d 'Password of the repository' -r
complete -c rustic -n "__fish_rustic_using_subcommand repoinfo" -s p -l password-file -d 'File to read the password from' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand repoinfo" -l password-command -d 'Command to read the password from. Password is read from stdout' -r
complete -c rustic -n "__fish_rustic_using_subcommand repoinfo" -l cache-dir -d 'Use this dir as cache dir instead of the standard cache dir' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand repoinfo" -l warm-up-command -d 'Warm up needed data pack files by running the command with %id replaced by pack id' -r
complete -c rustic -n "__fish_rustic_using_subcommand repoinfo" -l warm-up-wait -d 'Duration (e.g. 10m) to wait after warm up' -r
complete -c rustic -n "__fish_rustic_using_subcommand repoinfo" -l filter-host -d 'Hostname to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand repoinfo" -l filter-label -d 'Label to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand repoinfo" -l filter-paths -d 'Path list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand repoinfo" -l filter-tags -d 'Tag list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand repoinfo" -l filter-fn -d 'Function to filter snapshots' -r
complete -c rustic -n "__fish_rustic_using_subcommand repoinfo" -l only-files -d 'Only scan repository files (doesn\'t need repository password)'
complete -c rustic -n "__fish_rustic_using_subcommand repoinfo" -l only-index -d 'Only scan index'
complete -c rustic -n "__fish_rustic_using_subcommand repoinfo" -l json -d 'Show infos in json format'
complete -c rustic -n "__fish_rustic_using_subcommand repoinfo" -s n -l dry-run -d 'Only show what would be done without modifying anything. Does not affect read-only commands'
complete -c rustic -n "__fish_rustic_using_subcommand repoinfo" -l check-index -d 'Check if index matches pack files and read pack headers if neccessary'
complete -c rustic -n "__fish_rustic_using_subcommand repoinfo" -l no-progress -d 'Don\'t show any progress bar'
complete -c rustic -n "__fish_rustic_using_subcommand repoinfo" -l no-cache -d 'Don\'t use a cache'
complete -c rustic -n "__fish_rustic_using_subcommand repoinfo" -l warm-up -d 'Warm up needed data pack files by only requesting them without processing'
complete -c rustic -n "__fish_rustic_using_subcommand repoinfo" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c rustic -n "__fish_rustic_using_subcommand tag" -l add -d 'Tags to add (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand tag" -l remove -d 'Tags to remove (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand tag" -l set -d 'Tag list to set (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand tag" -l set-delete-after -d 'Mark snapshot to be deleted after given duration (e.g. 10d)' -r
complete -c rustic -n "__fish_rustic_using_subcommand tag" -s P -l use-profile -d 'Config profile to use. This parses the file `<PROFILE>.toml` in the config directory. [default: "rustic"]' -r
complete -c rustic -n "__fish_rustic_using_subcommand tag" -l log-level -d 'Use this log level [default: info]' -r
complete -c rustic -n "__fish_rustic_using_subcommand tag" -l log-file -d 'Write log messages to the given file instead of printing them' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand tag" -l progress-interval -d 'Interval to update progress bars' -r
complete -c rustic -n "__fish_rustic_using_subcommand tag" -s r -l repository -l repo -d 'Repository to use' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand tag" -l repo-hot -d 'Repository to use as hot storage' -r
complete -c rustic -n "__fish_rustic_using_subcommand tag" -l password -d 'Password of the repository' -r
complete -c rustic -n "__fish_rustic_using_subcommand tag" -s p -l password-file -d 'File to read the password from' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand tag" -l password-command -d 'Command to read the password from. Password is read from stdout' -r
complete -c rustic -n "__fish_rustic_using_subcommand tag" -l cache-dir -d 'Use this dir as cache dir instead of the standard cache dir' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand tag" -l warm-up-command -d 'Warm up needed data pack files by running the command with %id replaced by pack id' -r
complete -c rustic -n "__fish_rustic_using_subcommand tag" -l warm-up-wait -d 'Duration (e.g. 10m) to wait after warm up' -r
complete -c rustic -n "__fish_rustic_using_subcommand tag" -l filter-host -d 'Hostname to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand tag" -l filter-label -d 'Label to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand tag" -l filter-paths -d 'Path list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand tag" -l filter-tags -d 'Tag list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand tag" -l filter-fn -d 'Function to filter snapshots' -r
complete -c rustic -n "__fish_rustic_using_subcommand tag" -l remove-delete -d 'Remove any delete mark'
complete -c rustic -n "__fish_rustic_using_subcommand tag" -l set-delete-never -d 'Mark snapshot as uneraseable'
complete -c rustic -n "__fish_rustic_using_subcommand tag" -s n -l dry-run -d 'Only show what would be done without modifying anything. Does not affect read-only commands'
complete -c rustic -n "__fish_rustic_using_subcommand tag" -l check-index -d 'Check if index matches pack files and read pack headers if neccessary'
complete -c rustic -n "__fish_rustic_using_subcommand tag" -l no-progress -d 'Don\'t show any progress bar'
complete -c rustic -n "__fish_rustic_using_subcommand tag" -l no-cache -d 'Don\'t use a cache'
complete -c rustic -n "__fish_rustic_using_subcommand tag" -l warm-up -d 'Warm up needed data pack files by only requesting them without processing'
complete -c rustic -n "__fish_rustic_using_subcommand tag" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c rustic -n "__fish_rustic_using_subcommand webdav" -l address -d 'Address to bind the webdav server to. [default: "localhost:8000"]' -r
complete -c rustic -n "__fish_rustic_using_subcommand webdav" -l path-template -d 'The path template to use for snapshots. {id}, {id_long}, {time}, {username}, {hostname}, {label}, {tags}, {backup_start}, {backup_end} are replaced. [default: "[{hostname}]/[{label}]/{time}"]' -r
complete -c rustic -n "__fish_rustic_using_subcommand webdav" -l time-template -d 'The time template to use to display times in the path template. See https://docs.rs/chrono/latest/chrono/format/strftime/index.html for format options. [default: "%Y-%m-%d_%H-%M-%S"]' -r
complete -c rustic -n "__fish_rustic_using_subcommand webdav" -l file-access -d 'How to handle access to files. [default: "forbidden" for hot/cold repositories, else "read"]' -r
complete -c rustic -n "__fish_rustic_using_subcommand webdav" -s P -l use-profile -d 'Config profile to use. This parses the file `<PROFILE>.toml` in the config directory. [default: "rustic"]' -r
complete -c rustic -n "__fish_rustic_using_subcommand webdav" -l log-level -d 'Use this log level [default: info]' -r
complete -c rustic -n "__fish_rustic_using_subcommand webdav" -l log-file -d 'Write log messages to the given file instead of printing them' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand webdav" -l progress-interval -d 'Interval to update progress bars' -r
complete -c rustic -n "__fish_rustic_using_subcommand webdav" -s r -l repository -l repo -d 'Repository to use' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand webdav" -l repo-hot -d 'Repository to use as hot storage' -r
complete -c rustic -n "__fish_rustic_using_subcommand webdav" -l password -d 'Password of the repository' -r
complete -c rustic -n "__fish_rustic_using_subcommand webdav" -s p -l password-file -d 'File to read the password from' -r -F
complete -c rustic -n "__fish_rustic_using_subcommand webdav" -l password-command -d 'Command to read the password from. Password is read from stdout' -r
complete -c rustic -n "__fish_rustic_using_subcommand webdav" -l cache-dir -d 'Use this dir as cache dir instead of the standard cache dir' -r -f -a "(__fish_complete_directories)"
complete -c rustic -n "__fish_rustic_using_subcommand webdav" -l warm-up-command -d 'Warm up needed data pack files by running the command with %id replaced by pack id' -r
complete -c rustic -n "__fish_rustic_using_subcommand webdav" -l warm-up-wait -d 'Duration (e.g. 10m) to wait after warm up' -r
complete -c rustic -n "__fish_rustic_using_subcommand webdav" -l filter-host -d 'Hostname to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand webdav" -l filter-label -d 'Label to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand webdav" -l filter-paths -d 'Path list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand webdav" -l filter-tags -d 'Tag list to filter (can be specified multiple times)' -r
complete -c rustic -n "__fish_rustic_using_subcommand webdav" -l filter-fn -d 'Function to filter snapshots' -r
complete -c rustic -n "__fish_rustic_using_subcommand webdav" -l symlinks -d 'Use symlinks. This may not be supported by all WebDAV clients'
complete -c rustic -n "__fish_rustic_using_subcommand webdav" -s n -l dry-run -d 'Only show what would be done without modifying anything. Does not affect read-only commands'
complete -c rustic -n "__fish_rustic_using_subcommand webdav" -l check-index -d 'Check if index matches pack files and read pack headers if neccessary'
complete -c rustic -n "__fish_rustic_using_subcommand webdav" -l no-progress -d 'Don\'t show any progress bar'
complete -c rustic -n "__fish_rustic_using_subcommand webdav" -l no-cache -d 'Don\'t use a cache'
complete -c rustic -n "__fish_rustic_using_subcommand webdav" -l warm-up -d 'Warm up needed data pack files by only requesting them without processing'
complete -c rustic -n "__fish_rustic_using_subcommand webdav" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c rustic -n "__fish_rustic_using_subcommand help; and not __fish_seen_subcommand_from backup cat config completions check copy diff dump find forget init key list ls merge snapshots show-config self-update prune restore repair repoinfo tag webdav help" -f -a "backup" -d 'Backup to the repository'
complete -c rustic -n "__fish_rustic_using_subcommand help; and not __fish_seen_subcommand_from backup cat config completions check copy diff dump find forget init key list ls merge snapshots show-config self-update prune restore repair repoinfo tag webdav help" -f -a "cat" -d 'Show raw data of repository files and blobs'
complete -c rustic -n "__fish_rustic_using_subcommand help; and not __fish_seen_subcommand_from backup cat config completions check copy diff dump find forget init key list ls merge snapshots show-config self-update prune restore repair repoinfo tag webdav help" -f -a "config" -d 'Change the repository configuration'
complete -c rustic -n "__fish_rustic_using_subcommand help; and not __fish_seen_subcommand_from backup cat config completions check copy diff dump find forget init key list ls merge snapshots show-config self-update prune restore repair repoinfo tag webdav help" -f -a "completions" -d 'Generate shell completions'
complete -c rustic -n "__fish_rustic_using_subcommand help; and not __fish_seen_subcommand_from backup cat config completions check copy diff dump find forget init key list ls merge snapshots show-config self-update prune restore repair repoinfo tag webdav help" -f -a "check" -d 'Check the repository'
complete -c rustic -n "__fish_rustic_using_subcommand help; and not __fish_seen_subcommand_from backup cat config completions check copy diff dump find forget init key list ls merge snapshots show-config self-update prune restore repair repoinfo tag webdav help" -f -a "copy" -d 'Copy snapshots to other repositories. Note: The target repositories must be given in the config file!'
complete -c rustic -n "__fish_rustic_using_subcommand help; and not __fish_seen_subcommand_from backup cat config completions check copy diff dump find forget init key list ls merge snapshots show-config self-update prune restore repair repoinfo tag webdav help" -f -a "diff" -d 'Compare two snapshots/paths Note that the exclude options only apply for comparison with a local path'
complete -c rustic -n "__fish_rustic_using_subcommand help; and not __fish_seen_subcommand_from backup cat config completions check copy diff dump find forget init key list ls merge snapshots show-config self-update prune restore repair repoinfo tag webdav help" -f -a "dump" -d 'dump the contents of a file in a snapshot to stdout'
complete -c rustic -n "__fish_rustic_using_subcommand help; and not __fish_seen_subcommand_from backup cat config completions check copy diff dump find forget init key list ls merge snapshots show-config self-update prune restore repair repoinfo tag webdav help" -f -a "find" -d 'Find in given snapshots'
complete -c rustic -n "__fish_rustic_using_subcommand help; and not __fish_seen_subcommand_from backup cat config completions check copy diff dump find forget init key list ls merge snapshots show-config self-update prune restore repair repoinfo tag webdav help" -f -a "forget" -d 'Remove snapshots from the repository'
complete -c rustic -n "__fish_rustic_using_subcommand help; and not __fish_seen_subcommand_from backup cat config completions check copy diff dump find forget init key list ls merge snapshots show-config self-update prune restore repair repoinfo tag webdav help" -f -a "init" -d 'Initialize a new repository'
complete -c rustic -n "__fish_rustic_using_subcommand help; and not __fish_seen_subcommand_from backup cat config completions check copy diff dump find forget init key list ls merge snapshots show-config self-update prune restore repair repoinfo tag webdav help" -f -a "key" -d 'Manage keys'
complete -c rustic -n "__fish_rustic_using_subcommand help; and not __fish_seen_subcommand_from backup cat config completions check copy diff dump find forget init key list ls merge snapshots show-config self-update prune restore repair repoinfo tag webdav help" -f -a "list" -d 'List repository files'
complete -c rustic -n "__fish_rustic_using_subcommand help; and not __fish_seen_subcommand_from backup cat config completions check copy diff dump find forget init key list ls merge snapshots show-config self-update prune restore repair repoinfo tag webdav help" -f -a "ls" -d 'List file contents of a snapshot'
complete -c rustic -n "__fish_rustic_using_subcommand help; and not __fish_seen_subcommand_from backup cat config completions check copy diff dump find forget init key list ls merge snapshots show-config self-update prune restore repair repoinfo tag webdav help" -f -a "merge" -d 'Merge snapshots'
complete -c rustic -n "__fish_rustic_using_subcommand help; and not __fish_seen_subcommand_from backup cat config completions check copy diff dump find forget init key list ls merge snapshots show-config self-update prune restore repair repoinfo tag webdav help" -f -a "snapshots" -d 'Show a detailed overview of the snapshots within the repository'
complete -c rustic -n "__fish_rustic_using_subcommand help; and not __fish_seen_subcommand_from backup cat config completions check copy diff dump find forget init key list ls merge snapshots show-config self-update prune restore repair repoinfo tag webdav help" -f -a "show-config" -d 'Show the configuration which has been read from the config file(s)'
complete -c rustic -n "__fish_rustic_using_subcommand help; and not __fish_seen_subcommand_from backup cat config completions check copy diff dump find forget init key list ls merge snapshots show-config self-update prune restore repair repoinfo tag webdav help" -f -a "self-update" -d 'Update to the latest rustic release'
complete -c rustic -n "__fish_rustic_using_subcommand help; and not __fish_seen_subcommand_from backup cat config completions check copy diff dump find forget init key list ls merge snapshots show-config self-update prune restore repair repoinfo tag webdav help" -f -a "prune" -d 'Remove unused data or repack repository pack files'
complete -c rustic -n "__fish_rustic_using_subcommand help; and not __fish_seen_subcommand_from backup cat config completions check copy diff dump find forget init key list ls merge snapshots show-config self-update prune restore repair repoinfo tag webdav help" -f -a "restore" -d 'Restore a snapshot/path'
complete -c rustic -n "__fish_rustic_using_subcommand help; and not __fish_seen_subcommand_from backup cat config completions check copy diff dump find forget init key list ls merge snapshots show-config self-update prune restore repair repoinfo tag webdav help" -f -a "repair" -d 'Repair a snapshot/path'
complete -c rustic -n "__fish_rustic_using_subcommand help; and not __fish_seen_subcommand_from backup cat config completions check copy diff dump find forget init key list ls merge snapshots show-config self-update prune restore repair repoinfo tag webdav help" -f -a "repoinfo" -d 'Show general information about the repository'
complete -c rustic -n "__fish_rustic_using_subcommand help; and not __fish_seen_subcommand_from backup cat config completions check copy diff dump find forget init key list ls merge snapshots show-config self-update prune restore repair repoinfo tag webdav help" -f -a "tag" -d 'Change tags of snapshots'
complete -c rustic -n "__fish_rustic_using_subcommand help; and not __fish_seen_subcommand_from backup cat config completions check copy diff dump find forget init key list ls merge snapshots show-config self-update prune restore repair repoinfo tag webdav help" -f -a "webdav" -d 'Start a webdav server which allows to access the repository'
complete -c rustic -n "__fish_rustic_using_subcommand help; and not __fish_seen_subcommand_from backup cat config completions check copy diff dump find forget init key list ls merge snapshots show-config self-update prune restore repair repoinfo tag webdav help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rustic -n "__fish_rustic_using_subcommand help; and __fish_seen_subcommand_from cat" -f -a "tree-blob" -d 'Display a tree blob'
complete -c rustic -n "__fish_rustic_using_subcommand help; and __fish_seen_subcommand_from cat" -f -a "data-blob" -d 'Display a data blob'
complete -c rustic -n "__fish_rustic_using_subcommand help; and __fish_seen_subcommand_from cat" -f -a "config" -d 'Display the config file'
complete -c rustic -n "__fish_rustic_using_subcommand help; and __fish_seen_subcommand_from cat" -f -a "index" -d 'Display an index file'
complete -c rustic -n "__fish_rustic_using_subcommand help; and __fish_seen_subcommand_from cat" -f -a "snapshot" -d 'Display a snapshot file'
complete -c rustic -n "__fish_rustic_using_subcommand help; and __fish_seen_subcommand_from cat" -f -a "tree" -d 'Display a tree within a snapshot'
complete -c rustic -n "__fish_rustic_using_subcommand help; and __fish_seen_subcommand_from key" -f -a "add" -d 'Add a new key to the repository'
complete -c rustic -n "__fish_rustic_using_subcommand help; and __fish_seen_subcommand_from repair" -f -a "index" -d 'Repair the repository index'
complete -c rustic -n "__fish_rustic_using_subcommand help; and __fish_seen_subcommand_from repair" -f -a "snapshots" -d 'Repair snapshots'
