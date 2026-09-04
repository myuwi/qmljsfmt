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

  # oxfmt is needed to run the tests, makeWrapper to bake it into the binary.
  nativeBuildInputs = [
    makeWrapper
    oxfmt
  ];
  doCheck = true;

  overrideMain = _: {
    # qmljsfmt runs oxfmt as a subprocess and tracks one exact version of it.
    postInstall = ''
      wrapProgram $out/bin/qmljsfmt \
        --prefix PATH : ${lib.makeBinPath [ oxfmt ]}
    '';

    meta = {
      description = "Formats JavaScript embedded in QML documents";
      mainProgram = "qmljsfmt";
    };
  };
}
