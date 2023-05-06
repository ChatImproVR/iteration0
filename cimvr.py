#!/usr/bin/env python3
import os
from os.path import dirname, join, isfile
import argparse
from subprocess import Popen
from time import sleep
import shutil
import subprocess
import sys


# TODO: Enable this script to `cargo run` the server and client.


def main():
    parser = argparse.ArgumentParser(
        prog="ChatImproVR helper script",
        description="Launches the client and server, searches plugin paths",
        epilog="""
        Also searches the CIMVR_PLUGINS environment variable for WASM plugins.
        Multiple paths can be searched by seperating them with a semicolon (;) 
        """,
    )
    parser.add_argument(
        "plugins",
        nargs="*",
        help="""
        Plugins to launch. Plugins can be the truncated form
        ("thing.wasm" becomes "thing"), or full paths /home/me/plugin.wasm/...
        """,
    )
    parser.add_argument(
        "--client", "-c", action="store_true", help="Only run the client"
    )
    parser.add_argument("--username", "-u", help="Set username on client (--username)")
    parser.add_argument("--remote", "-r", help="Set remote host on client (--connect)")
    parser.add_argument(
        "--server", "-s", action="store_true", help="Only run the server"
    )
    parser.add_argument("--verbose", "-v", help="Verbose debug output")
    parser.add_argument("--vr", action="store_true", help="Run the client in VR mode")

    args = parser.parse_args()

    # cimvr new "subcommand" - workaround for argparse
    if len(args.plugins) == 2 and args.plugins[0] == "new":
        # This means you cannot have a plugin named "new" 
        # but if you do f*** you anyway
        create_new(args.plugins[1])
        return;

    # The script is assumed to be at the root of the project
    root_path = dirname(__file__)

    if args.verbose:
        print(f"Root path: {root_path}")

    # Client + Server behaviour
    if not args.client and not args.server:
        args.client = True
        args.server = True

    # Find executables
    server_exe = find_exe(
        "CIMVR_SERVER", ["cimvr_server", "cimvr_server.exe"], root_path
    )
    if args.verbose:
        print(f"Server exe: {server_exe}")
    if not server_exe:
        print("Failed to find server executable")
        return

    client_exe = find_exe(
        "CIMVR_CLIENT", ["cimvr_client", "cimvr_client.exe"], root_path
    )
    if args.verbose:
        print(f"Client exe: {client_exe}")
    if not client_exe:
        print("Failed to find client executable")
        return

    # Find all plugins
    plugins = []
    for name in args.plugins:
        # Just a file
        if isfile(name):
            plugins.append(name)
            continue

        # Truncated name of some search folder
        path = find_wasm(name, root_path)
        if path:
            plugins.append(path)
        else:
            print(f'No plugin named "{name}" found.')
            print("Searched:")
            for folder in get_plugin_folders(root_path):
                print("\t" + folder)
            return

    if args.verbose:
        print("Plugins:")
        for p in plugins:
            print(f"Plugin {p}")

    # Decide on a list of executables
    cmds = []
    if args.server:
        cmd = [server_exe] + plugins
        cmds += [cmd]

    if args.client:
        cmd = [client_exe] + plugins
        if args.vr:
            cmd.append("--vr")
        if args.remote:
            cmd.append("--connect")
            cmd.append(args.remote)
        if args.username:
            cmd.append("--username")
            cmd.append(args.username)
        cmds += [cmd]

    # Launch client an server
    procs = []
    for cmd in cmds:
        print(cmd)
        procs.append(Popen(cmd))
        # Wait for server to start
        sleep(0.1)

    for p in procs:
        p.wait()


def get_plugin_folders(root_path):
    """Search the build path, and a local "plugins" folder"""
    wasm_target = "wasm32-unknown-unknown"
    build_path = join(root_path, "target", wasm_target, "release")

    plugin_folders = [join(root_path, "plugins"), build_path]

    # Also check CIMVR_PLUGINS, which is a semicolon-seperated list
    wasm_env_var = "CIMVR_PLUGINS"
    if wasm_env_var in os.environ:
        plugin_path_list = os.environ[wasm_env_var].split(";")
        for path in plugin_path_list:
            # If the path is already in the list, don't add it again
            if path not in plugin_folders:
                plugin_folders.append(path)
                # print(f"Found {path}")

            target_release = join(path, "target", wasm_target, "release")
            if target_release not in plugin_folders:
                plugin_folders.append(target_release)
                # print(f"Found {target_release}")

    return plugin_folders


def find_wasm(name, root_path):
    plugin_folders = get_plugin_folders(root_path)
    file_name = name + ".wasm"

    for folder in plugin_folders:
        path = join(folder, file_name)
        if isfile(path):
            return path

    return None


def find_exe(env_var, names, root_path):
    """
    Look for the given environment variable, or try looking adjacent to the
    script, or in the build path adjacent the script. Returns None if it cannot
    find the exe.
    """
    if env_var in os.environ:
        return os.environ[env_var]
    else:
        build_path = join(root_path, "target", "release")
        client_build_path = join(root_path, "client", "target", "release")
        possible_locations = (
            [join(root_path, x) for x in names]
            + [join(build_path, x) for x in names]
            + [join(client_build_path, x) for x in names]
        )

        for path in possible_locations:
            if isfile(path):
                return path

    return None


def create_new(dir_name):
    # Courtesy of ChatGPT 5/6/2023
    REPO_URL = "https://github.com/ChatImproVR/template.git"
    CARGO_TOML_FILE = "Cargo.toml"
    CARGO_TOML_NAME_FIELD = "name = \"template_plugin\""

    # TODO: Is this barbaric?
    BASHRC_FILE = os.path.expanduser("~/.bashrc")
    BASHRC_EXPORT_LINE = "export CIMVR_PLUGINS=\"$CIMVR_PLUGINS;{}\""
    
    # Clone the git repository into the specified directory
    subprocess.run(["git", "clone", REPO_URL, dir_name], check=True)
    
    # Edit the name field in the Cargo.toml file
    cargo_toml_path = os.path.join(dir_name, CARGO_TOML_FILE)
    with open(cargo_toml_path, "r+") as f:
        content = f.read()
        name = os.path.basename(dir_name)
        new_content = content.replace(
            CARGO_TOML_NAME_FIELD, 
            f"name = \"{name}\""
        )
        f.seek(0)
        f.write(new_content)
        f.truncate()

    shutil
    print("Edited Cargo.toml")
    
    # Append the export line to the user's .bashrc file
    bashrc_export_line = BASHRC_EXPORT_LINE.format(os.path.abspath(dir_name))
    with open(BASHRC_FILE, "a") as f:
        f.write("\n" + bashrc_export_line + "\n")

    to_delete = os.path.join(dir_name, ".git")
    #if input(f"Remove template git path {to_delete}? [y/N] ") == 'y':
    print(f"Deleting {to_delete}")
    shutil.rmtree(to_delete);
    
    print("Done.")


if __name__ == "__main__":
    main()
