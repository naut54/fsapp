# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_fset_global_optspecs
    string join \n config= h/help V/version
end

function __fish_fset_needs_command
    # Figure out if the current invocation already has a command.
    set -l cmd (commandline -opc)
    set -e cmd[1]
    argparse -s (__fish_fset_global_optspecs) -- $cmd 2>/dev/null
    or return
    if set -q argv[1]
        # Also print the command, so this can be used to figure out what it is.
        echo $argv[1]
        return 1
    end
    return 0
end

function __fish_fset_using_subcommand
    set -l cmd (__fish_fset_needs_command)
    test -z "$cmd"
    and return 1
    contains -- $cmd[1] $argv
end

complete -c fset -n "__fish_fset_needs_command" -l config -d 'Override the config file location for this invocation' -r -F
complete -c fset -n "__fish_fset_needs_command" -s h -l help -d 'Print help'
complete -c fset -n "__fish_fset_needs_command" -s V -l version -d 'Print version'
complete -c fset -n "__fish_fset_needs_command" -f -a "get" -d 'Print the current value, or "unset" if absent'
complete -c fset -n "__fish_fset_needs_command" -f -a "set" -d 'Set a value, validated against the same types fsapp\'s CLI uses'
complete -c fset -n "__fish_fset_needs_command" -f -a "unset" -d 'Remove a key — reverts to the file-engine builder default'
complete -c fset -n "__fish_fset_needs_command" -f -a "list" -d 'Dump current JSON (optionally scoped to one section)'
complete -c fset -n "__fish_fset_needs_command" -f -a "path" -d 'Print the resolved config file path'
complete -c fset -n "__fish_fset_needs_command" -f -a "edit" -d 'Open $EDITOR on the file; re-validate before saving'
complete -c fset -n "__fish_fset_needs_command" -f -a "reset" -d 'Reset the whole file (or one section) to {}; always backs up first'
complete -c fset -n "__fish_fset_needs_command" -f -a "completions" -d 'Print a shell completion script, or install it with --install'
complete -c fset -n "__fish_fset_needs_command" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c fset -n "__fish_fset_using_subcommand get" -l config -d 'Override the config file location for this invocation' -r -F
complete -c fset -n "__fish_fset_using_subcommand get" -s h -l help -d 'Print help'
complete -c fset -n "__fish_fset_using_subcommand set" -l config -d 'Override the config file location for this invocation' -r -F
complete -c fset -n "__fish_fset_using_subcommand set" -s h -l help -d 'Print help'
complete -c fset -n "__fish_fset_using_subcommand unset" -l config -d 'Override the config file location for this invocation' -r -F
complete -c fset -n "__fish_fset_using_subcommand unset" -s h -l help -d 'Print help'
complete -c fset -n "__fish_fset_using_subcommand list" -l config -d 'Override the config file location for this invocation' -r -F
complete -c fset -n "__fish_fset_using_subcommand list" -s h -l help -d 'Print help'
complete -c fset -n "__fish_fset_using_subcommand path" -l config -d 'Override the config file location for this invocation' -r -F
complete -c fset -n "__fish_fset_using_subcommand path" -s h -l help -d 'Print help'
complete -c fset -n "__fish_fset_using_subcommand edit" -l config -d 'Override the config file location for this invocation' -r -F
complete -c fset -n "__fish_fset_using_subcommand edit" -s h -l help -d 'Print help'
complete -c fset -n "__fish_fset_using_subcommand reset" -l config -d 'Override the config file location for this invocation' -r -F
complete -c fset -n "__fish_fset_using_subcommand reset" -s h -l help -d 'Print help'
complete -c fset -n "__fish_fset_using_subcommand completions" -l dir -d 'Install into this directory instead of searching. Implies --install' -r -F
complete -c fset -n "__fish_fset_using_subcommand completions" -l config -d 'Override the config file location for this invocation' -r -F
complete -c fset -n "__fish_fset_using_subcommand completions" -l install -d 'Write the script into the shell\'s completion directory'
complete -c fset -n "__fish_fset_using_subcommand completions" -s h -l help -d 'Print help'
complete -c fset -n "__fish_fset_using_subcommand help; and not __fish_seen_subcommand_from get set unset list path edit reset completions help" -f -a "get" -d 'Print the current value, or "unset" if absent'
complete -c fset -n "__fish_fset_using_subcommand help; and not __fish_seen_subcommand_from get set unset list path edit reset completions help" -f -a "set" -d 'Set a value, validated against the same types fsapp\'s CLI uses'
complete -c fset -n "__fish_fset_using_subcommand help; and not __fish_seen_subcommand_from get set unset list path edit reset completions help" -f -a "unset" -d 'Remove a key — reverts to the file-engine builder default'
complete -c fset -n "__fish_fset_using_subcommand help; and not __fish_seen_subcommand_from get set unset list path edit reset completions help" -f -a "list" -d 'Dump current JSON (optionally scoped to one section)'
complete -c fset -n "__fish_fset_using_subcommand help; and not __fish_seen_subcommand_from get set unset list path edit reset completions help" -f -a "path" -d 'Print the resolved config file path'
complete -c fset -n "__fish_fset_using_subcommand help; and not __fish_seen_subcommand_from get set unset list path edit reset completions help" -f -a "edit" -d 'Open $EDITOR on the file; re-validate before saving'
complete -c fset -n "__fish_fset_using_subcommand help; and not __fish_seen_subcommand_from get set unset list path edit reset completions help" -f -a "reset" -d 'Reset the whole file (or one section) to {}; always backs up first'
complete -c fset -n "__fish_fset_using_subcommand help; and not __fish_seen_subcommand_from get set unset list path edit reset completions help" -f -a "completions" -d 'Print a shell completion script, or install it with --install'
complete -c fset -n "__fish_fset_using_subcommand help; and not __fish_seen_subcommand_from get set unset list path edit reset completions help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
