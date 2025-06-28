{
	nixpkgs ? import <nixpkgs> {},
}: let
	inherit (nixpkgs) lib pkgs pkgsStatic;

	buildInputs = with pkgsStatic; [
		openssl
	];
	nativeBuildInputs = with pkgs; [
		pkg-config
	];
in {
	seedbrr = pkgsStatic.rustPlatform.buildRustPackage {
		name = "seedbrr";

		cargoLock.lockFile = ./Cargo.lock;
		src = lib.fileset.toSource {
			root = ./.;
			fileset = lib.fileset.unions [
				./Cargo.lock
				./Cargo.toml
				./src
			];
		};

		inherit buildInputs nativeBuildInputs;

		dontStrip = true;
	};

	shell = pkgs.mkShell {
		buildInputs = buildInputs;
		nativeBuildInputs = nativeBuildInputs ++ (with pkgs; [
			cargo
		]);
	};
}
