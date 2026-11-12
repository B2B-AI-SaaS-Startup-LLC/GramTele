{ pkgs ? import (let
    lock = builtins.fromJSON (builtins.readFile ./flake.lock);
    pinned = lock.nodes.nixpkgs.locked;
  in builtins.fetchTarball {
    url = "https://github.com/NixOS/nixpkgs/archive/${pinned.rev}.tar.gz";
    sha256 = pinned.narHash;
  }) { } }:
pkgs.mkShell {
  packages = with pkgs; [ cargo rustc clippy rustfmt gcc pkg-config ];
}
