# Regras deste projeto

- Sempre informar os caminhos completos dos arquivos atualizados na resposta final.
- Incrementar a versão em `Cargo.toml` em cada nova entrega que altere o comportamento, os dados ou o HTML incorporado ao mod e gere uma nova DLL. Uma recompilação da mesma entrega não exige outro incremento.
- Usar SemVer: PATCH para correções, MINOR para funcionalidades compatíveis, MAJOR para mudanças incompatíveis. Depois de 2.0.0, uma correção deve ser 2.0.1 e uma nova funcionalidade compatível deve ser 2.1.0.
- Manter `Cargo.toml` como fonte da versão: o log, o heartbeat e os metadados do Shroudtopia usam `CARGO_PKG_VERSION`. Atualizar `Cargo.lock` pelo Cargo e os dois changelogs na mesma entrega.
- Registrar no changelog somente o que foi implementado; distinguir testes automatizados e testes dentro do jogo.
- Ao instalar, preservar backup da DLL anterior, conferir o hash instalado e não parar um servidor em execução sem autorização.
