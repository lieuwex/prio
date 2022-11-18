with (import (fetchTarball "https://github.com/NixOS/nixpkgs/archive/7cd0de9b17d00965770d9fc2825a34a7f13fd310.tar.gz") {});

pkgs.mkShell {
	buildInputs = [
		rust-bin.stable.latest.complete
		cargo-outdated

		openssl
		pkg-config
	];
}
