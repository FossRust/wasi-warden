{
  description = "wasi-warden development shell";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.05"; 
    rust-overlay.url = "github:oxalica/rust-overlay"; 
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs"; 
  };

  outputs = { self, nixpkgs, rust-overlay }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAllSystems = f: 
        builtins.listToAttrs (map (system: { name = system; value = f system; }) systems); 
    in { 
      devShells = forAllSystems (system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ (import rust-overlay) ];
          };
          # --- MODIFIED LINE ---
          # We override the default toolchain to add the wasm32-wasip2 target.
          rustToolchain = pkgs.rust-bin.stable."1.91.1".default.override {
            targets = [ "wasm32-wasip2" ];
          };
          # --- END MODIFICATION ---
        in {
       
           default = pkgs.mkShell { 
            packages = [
              rustToolchain
              pkgs.pkg-config
              pkgs.openssl
              pkgs.protobuf
              pkgs.ollama
            ]; 
            
            RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library"; 
          };
        });
    }; 
}
