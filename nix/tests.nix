{ pkgs, eva }:
let
  inherit (pkgs.lib) any hasInfix mkOption types;
  dummy = args: eva.mkModule ({ self = null; description = "dummy"; } // args);
  eval = service: pkgs.lib.evalModules {
    specialArgs = { inherit pkgs; };
    modules = [
      {
        options.assertions = mkOption { type = types.listOf types.unspecified; default = [ ]; };
        options.systemd.services = mkOption {
          type = types.attrsOf (types.submodule {
            freeformType = types.attrsOf types.anything;
            options.serviceConfig = mkOption {
              type = types.submodule {
                freeformType = types.attrsOf types.anything;
                options.LoadCredential = mkOption { type = types.listOf types.str; default = [ ]; };
              };
            };
          });
        };
      }
      (dummy {
        pname = "dummy";
        secretFields = [ "api_key" "tokens.*.token" ];
        extraServiceConfig.LoadCredential = [ "extra:/run/secrets/extra" ];
      })
      (dummy { pname = "plain"; })
      { services.dummy = { enable = true; package = pkgs.emptyDirectory; } // service; }
    ];
  };
  failed = result: map (a: a.message)
    (builtins.filter (a: !a.assertion) result.config.assertions);
  creds = "/run/credentials/dummy.service";
  good = eval {
    settings.tokens.phone.name = "phone";
    secrets = {
      api_key = "/run/secrets/key";
      "tokens.phone.token" = "/run/secrets/phone";
    };
  };
  bad = failed (eval {
    settings.tokens.phone.token.plaintext = "hunter2";
    secrets = {
      "tokens.phone.tokn" = "/run/secrets/phone";
      "tokens.a:b.token" = "/run/secrets/ab";
    };
  });
in
{
  mkModule-secrets =
    assert failed good == [ ];
    assert good.config.services.dummy.settings == {
      api_key.file = "${creds}/api_key";
      tokens.phone = { name = "phone"; token.file = "${creds}/tokens.phone.token"; };
    };
    assert good.config.systemd.services.dummy.serviceConfig.LoadCredential
      == [ "api_key:/run/secrets/key" "tokens.phone.token:/run/secrets/phone" "extra:/run/secrets/extra" ];
    assert builtins.length bad == 3;
    assert any (hasInfix "settings.tokens.phone.token: plaintext") bad;
    assert any (hasInfix "secrets.\"tokens.phone.tokn\" matches no secret field") bad;
    assert any (hasInfix "secrets.\"tokens.a:b.token\" matches no secret field") bad;
    assert good.options.services.dummy ? secrets;
    assert !(good.options.services.plain ? secrets);
    pkgs.runCommand "mkModule-secrets" { } "touch $out";
}
