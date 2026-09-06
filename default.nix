{
  lib,
  makeWrapper,
  naersk,
  oxfmt,
}:

naersk.buildPackage {
  pname = "qmljsfmt";
  version = "0.1.0";

  src = ./.;

  nativeBuildInputs = [
    makeWrapper
    oxfmt
  ];
  doCheck = true;

  overrideMain = _: {
    postInstall = ''
      wrapProgram $out/bin/qmljsfmt \
        --prefix PATH : ${lib.makeBinPath [ oxfmt ]}
    '';

    meta = {
      description = "Formats JavaScript embedded in QML documents";
      license = lib.licenses.isc;
      mainProgram = "qmljsfmt";
    };
  };
}
