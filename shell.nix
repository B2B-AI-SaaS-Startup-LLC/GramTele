{ pkgs ? import <nixpkgs> { } }:
pkgs.mkShell {
  packages = with pkgs; [ cargo rustc clippy rustfmt gcc pkg-config ];
}
