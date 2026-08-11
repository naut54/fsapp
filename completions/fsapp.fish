# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_fsapp_global_optspecs
    string join \n v q/quiet config= no-update-check h/help V/version
end

function __fish_fsapp_needs_command
    # Figure out if the current invocation already has a command.
    set -l cmd (commandline -opc)
    set -e cmd[1]
    argparse -s (__fish_fsapp_global_optspecs) -- $cmd 2>/dev/null
    or return
    if set -q argv[1]
        # Also print the command, so this can be used to figure out what it is.
        echo $argv[1]
        return 1
    end
    return 0
end

function __fish_fsapp_using_subcommand
    set -l cmd (__fish_fsapp_needs_command)
    test -z "$cmd"
    and return 1
    contains -- $cmd[1] $argv
end

complete -c fsapp -n "__fish_fsapp_needs_command" -l config -d 'Override the config file location for this invocation' -r -F
complete -c fsapp -n "__fish_fsapp_needs_command" -s v -d '-v info, -vv debug, -vvv trace (default: warn)'
complete -c fsapp -n "__fish_fsapp_needs_command" -s q -l quiet -d 'Suppress the progress bar; logging still follows -v'
complete -c fsapp -n "__fish_fsapp_needs_command" -l no-update-check -d 'Skip the automatic check for a newer fsapp release'
complete -c fsapp -n "__fish_fsapp_needs_command" -s h -l help -d 'Print help'
complete -c fsapp -n "__fish_fsapp_needs_command" -s V -l version -d 'Print version'
complete -c fsapp -n "__fish_fsapp_needs_command" -f -a "copy" -d 'Copy files from SOURCE to DEST'
complete -c fsapp -n "__fish_fsapp_needs_command" -f -a "mv" -d 'Move files from SOURCE to DEST'
complete -c fsapp -n "__fish_fsapp_needs_command" -f -a "sync" -d 'Sync DEST to match SOURCE (copies changes, deletes orphans)'
complete -c fsapp -n "__fish_fsapp_needs_command" -f -a "watch" -d 'Watch PATH for filesystem changes and print events until Ctrl+C'
complete -c fsapp -n "__fish_fsapp_needs_command" -f -a "compress" -d 'Compress SOURCE into an archive at DEST'
complete -c fsapp -n "__fish_fsapp_needs_command" -f -a "update-check" -d 'Check whether a newer fsapp release is available'
complete -c fsapp -n "__fish_fsapp_needs_command" -f -a "completions" -d 'Print a shell completion script, or install it with --install'
complete -c fsapp -n "__fish_fsapp_needs_command" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c fsapp -n "__fish_fsapp_using_subcommand copy" -l small-file-threshold -r
complete -c fsapp -n "__fish_fsapp_using_subcommand copy" -l batch-concurrency -r
complete -c fsapp -n "__fish_fsapp_using_subcommand copy" -l on-error -r -f -a "continue\t''
abort\t''
undo\t''"
complete -c fsapp -n "__fish_fsapp_using_subcommand copy" -l max-bytes-per-batch -r
complete -c fsapp -n "__fish_fsapp_using_subcommand copy" -l max-files-per-batch -r
complete -c fsapp -n "__fish_fsapp_using_subcommand copy" -l sort-order -r -f -a "asc\t''
desc\t''"
complete -c fsapp -n "__fish_fsapp_using_subcommand copy" -l config -d 'Override the config file location for this invocation' -r -F
complete -c fsapp -n "__fish_fsapp_using_subcommand copy" -l preserve-permissions
complete -c fsapp -n "__fish_fsapp_using_subcommand copy" -l allow-fs-integrity-risk
complete -c fsapp -n "__fish_fsapp_using_subcommand copy" -l overwrite
complete -c fsapp -n "__fish_fsapp_using_subcommand copy" -s v -d '-v info, -vv debug, -vvv trace (default: warn)'
complete -c fsapp -n "__fish_fsapp_using_subcommand copy" -s q -l quiet -d 'Suppress the progress bar; logging still follows -v'
complete -c fsapp -n "__fish_fsapp_using_subcommand copy" -l no-update-check -d 'Skip the automatic check for a newer fsapp release'
complete -c fsapp -n "__fish_fsapp_using_subcommand copy" -s h -l help -d 'Print help'
complete -c fsapp -n "__fish_fsapp_using_subcommand mv" -l small-file-threshold -r
complete -c fsapp -n "__fish_fsapp_using_subcommand mv" -l batch-concurrency -r
complete -c fsapp -n "__fish_fsapp_using_subcommand mv" -l on-error -r -f -a "continue\t''
abort\t''
undo\t''"
complete -c fsapp -n "__fish_fsapp_using_subcommand mv" -l config -d 'Override the config file location for this invocation' -r -F
complete -c fsapp -n "__fish_fsapp_using_subcommand mv" -l preserve-permissions
complete -c fsapp -n "__fish_fsapp_using_subcommand mv" -l allow-fs-integrity-risk
complete -c fsapp -n "__fish_fsapp_using_subcommand mv" -l overwrite
complete -c fsapp -n "__fish_fsapp_using_subcommand mv" -s v -d '-v info, -vv debug, -vvv trace (default: warn)'
complete -c fsapp -n "__fish_fsapp_using_subcommand mv" -s q -l quiet -d 'Suppress the progress bar; logging still follows -v'
complete -c fsapp -n "__fish_fsapp_using_subcommand mv" -l no-update-check -d 'Skip the automatic check for a newer fsapp release'
complete -c fsapp -n "__fish_fsapp_using_subcommand mv" -s h -l help -d 'Print help'
complete -c fsapp -n "__fish_fsapp_using_subcommand sync" -l small-file-threshold -r
complete -c fsapp -n "__fish_fsapp_using_subcommand sync" -l batch-concurrency -r
complete -c fsapp -n "__fish_fsapp_using_subcommand sync" -l on-error -r -f -a "continue\t''
abort\t''
undo\t''"
complete -c fsapp -n "__fish_fsapp_using_subcommand sync" -l config -d 'Override the config file location for this invocation' -r -F
complete -c fsapp -n "__fish_fsapp_using_subcommand sync" -l preserve-permissions
complete -c fsapp -n "__fish_fsapp_using_subcommand sync" -l allow-fs-integrity-risk
complete -c fsapp -n "__fish_fsapp_using_subcommand sync" -l no-overwrite -d 'Inverts the builder\'s default of `true`'
complete -c fsapp -n "__fish_fsapp_using_subcommand sync" -l checksum
complete -c fsapp -n "__fish_fsapp_using_subcommand sync" -s v -d '-v info, -vv debug, -vvv trace (default: warn)'
complete -c fsapp -n "__fish_fsapp_using_subcommand sync" -s q -l quiet -d 'Suppress the progress bar; logging still follows -v'
complete -c fsapp -n "__fish_fsapp_using_subcommand sync" -l no-update-check -d 'Skip the automatic check for a newer fsapp release'
complete -c fsapp -n "__fish_fsapp_using_subcommand sync" -s h -l help -d 'Print help'
complete -c fsapp -n "__fish_fsapp_using_subcommand watch" -l config -d 'Override the config file location for this invocation' -r -F
complete -c fsapp -n "__fish_fsapp_using_subcommand watch" -l no-recursive -d 'Inverts the builder\'s default of `true`'
complete -c fsapp -n "__fish_fsapp_using_subcommand watch" -s v -d '-v info, -vv debug, -vvv trace (default: warn)'
complete -c fsapp -n "__fish_fsapp_using_subcommand watch" -s q -l quiet -d 'Suppress the progress bar; logging still follows -v'
complete -c fsapp -n "__fish_fsapp_using_subcommand watch" -l no-update-check -d 'Skip the automatic check for a newer fsapp release'
complete -c fsapp -n "__fish_fsapp_using_subcommand watch" -s h -l help -d 'Print help'
complete -c fsapp -n "__fish_fsapp_using_subcommand compress" -l small-file-threshold -r
complete -c fsapp -n "__fish_fsapp_using_subcommand compress" -l batch-concurrency -r
complete -c fsapp -n "__fish_fsapp_using_subcommand compress" -l on-error -r -f -a "continue\t''
abort\t''
undo\t''"
complete -c fsapp -n "__fish_fsapp_using_subcommand compress" -l format -d 'Inferred from DEST\'s extension if omitted' -r -f -a "zip\t''
gzip\t''"
complete -c fsapp -n "__fish_fsapp_using_subcommand compress" -l config -d 'Override the config file location for this invocation' -r -F
complete -c fsapp -n "__fish_fsapp_using_subcommand compress" -s v -d '-v info, -vv debug, -vvv trace (default: warn)'
complete -c fsapp -n "__fish_fsapp_using_subcommand compress" -s q -l quiet -d 'Suppress the progress bar; logging still follows -v'
complete -c fsapp -n "__fish_fsapp_using_subcommand compress" -l no-update-check -d 'Skip the automatic check for a newer fsapp release'
complete -c fsapp -n "__fish_fsapp_using_subcommand compress" -s h -l help -d 'Print help'
complete -c fsapp -n "__fish_fsapp_using_subcommand update-check" -l config -d 'Override the config file location for this invocation' -r -F
complete -c fsapp -n "__fish_fsapp_using_subcommand update-check" -s v -d '-v info, -vv debug, -vvv trace (default: warn)'
complete -c fsapp -n "__fish_fsapp_using_subcommand update-check" -s q -l quiet -d 'Suppress the progress bar; logging still follows -v'
complete -c fsapp -n "__fish_fsapp_using_subcommand update-check" -l no-update-check -d 'Skip the automatic check for a newer fsapp release'
complete -c fsapp -n "__fish_fsapp_using_subcommand update-check" -s h -l help -d 'Print help'
complete -c fsapp -n "__fish_fsapp_using_subcommand completions" -l dir -d 'Install into this directory instead of searching. Implies --install' -r -F
complete -c fsapp -n "__fish_fsapp_using_subcommand completions" -l config -d 'Override the config file location for this invocation' -r -F
complete -c fsapp -n "__fish_fsapp_using_subcommand completions" -l install -d 'Write the script into the shell\'s completion directory'
complete -c fsapp -n "__fish_fsapp_using_subcommand completions" -s v -d '-v info, -vv debug, -vvv trace (default: warn)'
complete -c fsapp -n "__fish_fsapp_using_subcommand completions" -s q -l quiet -d 'Suppress the progress bar; logging still follows -v'
complete -c fsapp -n "__fish_fsapp_using_subcommand completions" -l no-update-check -d 'Skip the automatic check for a newer fsapp release'
complete -c fsapp -n "__fish_fsapp_using_subcommand completions" -s h -l help -d 'Print help'
complete -c fsapp -n "__fish_fsapp_using_subcommand help; and not __fish_seen_subcommand_from copy mv sync watch compress update-check completions help" -f -a "copy" -d 'Copy files from SOURCE to DEST'
complete -c fsapp -n "__fish_fsapp_using_subcommand help; and not __fish_seen_subcommand_from copy mv sync watch compress update-check completions help" -f -a "mv" -d 'Move files from SOURCE to DEST'
complete -c fsapp -n "__fish_fsapp_using_subcommand help; and not __fish_seen_subcommand_from copy mv sync watch compress update-check completions help" -f -a "sync" -d 'Sync DEST to match SOURCE (copies changes, deletes orphans)'
complete -c fsapp -n "__fish_fsapp_using_subcommand help; and not __fish_seen_subcommand_from copy mv sync watch compress update-check completions help" -f -a "watch" -d 'Watch PATH for filesystem changes and print events until Ctrl+C'
complete -c fsapp -n "__fish_fsapp_using_subcommand help; and not __fish_seen_subcommand_from copy mv sync watch compress update-check completions help" -f -a "compress" -d 'Compress SOURCE into an archive at DEST'
complete -c fsapp -n "__fish_fsapp_using_subcommand help; and not __fish_seen_subcommand_from copy mv sync watch compress update-check completions help" -f -a "update-check" -d 'Check whether a newer fsapp release is available'
complete -c fsapp -n "__fish_fsapp_using_subcommand help; and not __fish_seen_subcommand_from copy mv sync watch compress update-check completions help" -f -a "completions" -d 'Print a shell completion script, or install it with --install'
complete -c fsapp -n "__fish_fsapp_using_subcommand help; and not __fish_seen_subcommand_from copy mv sync watch compress update-check completions help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
