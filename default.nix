{
  lib,
  makeWrapper,
  oxfmt,
  rustPlatform,
}:

rustPlatform.buildRustPackage {
  pname = "qmljsfmt";
  version = "0.1.0";

  src = ./.;
  cargoLock.lockFile = ./Cargo.lock;

  nativeBuildInputs = [
    makeWrapper
    oxfmt
  ];
  doCheck = true;

  postInstall = ''
    wrapProgram $out/bin/qmljsfmt \
      --prefix PATH : ${lib.makeBinPath [ oxfmt ]}
  '';

  meta = {
    description = "Formats JavaScript embedded in QML documents";
    license = lib.licenses.isc;
    mainProgram = "qmljsfmt";
  };
}
