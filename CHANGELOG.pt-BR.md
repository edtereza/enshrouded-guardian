# Mudanças

*Read this in [English](CHANGELOG.md).*

## 3.0.9 — 20/11/2026

- Conflito guardian.json/guardiao.json resolvido por merge recursivo de chaves normalizadas. Valores do arquivo com última modificação mais antiga prevalecem (legado vence empate); listas ficam inteiras.
- Ambos os originais preservados separadamente antes de atualizar guardian.json e remover guardiao.json da pasta ativa. Backups anteriores da migração no mod não são sobrescritos. BOM UTF-8 aceito; JSON inválido/raiz não objeto mantém os originais intactos.
- Helper offline aplica as mesmas regras e não sobrescreve um backup de configuração com o outro. Validação automatizada local: 26 testes Rust e 4 testes Node passaram, cobrindo precedência, aliases, backups, idempotência e entradas inválidas. Sem teste dentro do jogo.

## 3.0.8 — 20/11/2026

- Build e Releases disponibilizam guardian-{versão}.zip (somente Guardian/Shroudtopia) e guardian-{versão}-full.zip (inclui runtime Visual C++ x64 local e aviso).
- Ambos validam listas de arquivos, hashes e manifestos independentes; SHA256SUMS.txt cobre os dois ZIPs e guardian.dll.
- Testes automatizados e validação dos pacotes pelo GitHub Actions; sem mudança de jogabilidade ou novo teste dentro do jogo.

## 3.0.7 — 20/11/2026

- Removidas DLLs do runtime Microsoft Visual C++ e seu aviso do ZIP. Mantidos somente arquivos do Guardian/Shroudtopia, configurações base, instruções, licenças e manifesto de hashes.
- Empacotamento valida lista explícita de arquivos permitidos, entradas e hashes do ZIP. Visual C++ Redistributable x64 documentado como pré-requisito separado do Shroudtopia.
- GitHub Actions executa testes automatizados e validação do pacote; sem mudança de jogabilidade ou novo teste dentro do jogo.

## 3.0.6 — 20/11/2026

- Artefato chamado guardian; o pacote completo usa o nome guardian-{versão}.zip. Pacotes gerados exclusivamente pelo GitHub Actions, mantidos como artefatos por 90 dias e publicados automaticamente em Releases para tags de versão correspondentes.
- Removidos instaladores obsoletos e caminhos pessoais do helper de build. Hashes do loader conferidos contra Shroudtopia oficial 0.1.1.

- GitHub Actions testa/compila somente Shroudtopia e gera guardian.dll, ZIP para a raiz do servidor e hashes SHA-256. Tags são conferidas contra Cargo.toml.
- ZIP inclui loader existente com hashes fixados, runtime Visual C++ local, configurações base, licenças e manifesto verificado. Procedência do loader documentada.
- Removido save de exemplo do template HTML incorporado.
- Testes automatizados locais e verificação dos hashes do ZIP; sem afirmar novo teste dentro do jogo ou execução pelo GitHub.

## 3.0.5 — 20/11/2026

- Relatório HTML renomeado para server_save.html. Ajustadas a geração, a ferramenta offline, a documentação e a migração de character.html/personagem.html.
- Testes automatizados de persistência e migração conferem o novo nome; sem mudança de jogabilidade.

## 3.0.4 — 20/11/2026

- Separadas a validação dos patterns principais e a compatibilidade dos recursos extras com layout fixo. Builds divergentes podem instalar os hooks principais se todos os patterns e verificações dos pontos de chamada forem aprovados.
- Divergência registrada no log nativo do servidor, guardian.log e console. Chat, leitura do nome e kick com offsets fixos ficam desativados em builds não verificadas.
- Travas nos callbacks impedem acessos do Guardian à memória quando os hooks estão desativados ou a instalação falha, mantendo as chamadas originais. Testes automatizados das travas; sem afirmar validação dentro do jogo em build divergente.

## 3.0.3 — 20/11/2026

- Verificação de atualizações, avisos de download, identificação no boot e link de suporte direcionados a edtereza/enshrouded-guardian, preservando os créditos originais.
- Respostas de releases lidas como JSON; rascunhos e pré-lançamentos ignorados. Ausência de release ou indisponibilidade da consulta registrada no log. Mantidos checkUpdates e os intervalos existentes.
- Teste automatizado do repositório, respostas e comparação de versões. Esta alteração não publica código nem cria releases no GitHub.

## 3.0.2 — 20/11/2026

- Modelos agrupados em templates/: save-viewer.html, guardian.example.json e initial-character/A.bin e K.bin. Referências de compilação, migração e documentação ajustadas sem alterar o conteúdo dos modelos.
- Recompilação e testes automatizados para conferir os novos caminhos; comportamento do jogo inalterado.

## 3.0.1 — 20/11/2026

- Corrigidos os textos em português ainda presentes em `src`, incluindo erros de busca dos ganchos. Removidas as traduções antigas e o seletor de tradução que já retornava apenas inglês.
- Módulos renomeados: access.rs, scan.rs, updates.rs, continuity.rs, state.rs, install.rs e logging.rs. Referências e imports atualizados; mantidos os anúncios de chat solicitados em português e os nomes antigos necessários à migração.
- Correção complementar da padronização em inglês da versão 3.0.0. Testes automatizados executados nesta entrega; sem nova validação com jogador conectado.

## 3.0.0 — 20/11/2026

- Nomes e esquemas de armazenamento em inglês: guardian.json, guardian.log, events.jsonl, heartbeat.json, client_save.json, server_save.json e character.html; pastas history, admin-history, original-first-join e rejected; marcador OUTDATED.
- Campos, eventos, decisões e mensagens técnicas passam a inglês. Configurações antigas e JSONs são migrados na inicialização; originais e logs históricos ficam preservados em legacy-backup, sem reescrever mensagens históricas como se fossem traduções verificadas. Saves binários e nomes dos jogadores não são traduzidos.
- Visualizador estático em inglês (save-viewer.html) e DLL Shroudtopia identificada como Guardian, instalada como guardian.dll. Anúncios do chat definidos pelo usuário continuam em português; logs técnicos registram o evento em inglês sem repetir seu texto.
- Atualização dos scripts e documentação. Alteração incompatível de caminhos/esquemas; migração preserva dados e recusa conflitos.

## 2.3.0 — 20/11/2026

- Whitelist usa `players` no lugar de `steamIds`; a chave antiga permanece como compatibilidade quando `players` não existe.
- Armazenamento passa a usar `guardian/`. Na inicialização, a pasta antiga é renomeada sem descartar arquivos; se ambas existirem, a ativação é interrompida para não sobrescrever cópias.
- Documentação, configuração de exemplo e scripts Windows atualizados. Teste automatizado de preservação dos dados na migração e recusa de conflitos; teste dentro do jogo pendente.

## 2.2.0 — 20/11/2026

- Quando a whitelist está desativada, cada conexão aceita registra SteamID64 e nome Steam no console, guardiao.log e no evento `conexao_sem_whitelist` de registro.jsonl, incluindo reconexões e administradores. Não adiciona IDs às listas.
- Nomes são lidos no evento nativo de entrada; nomes ilegíveis são identificados como indisponíveis e caracteres de controle são removidos. Captura depende do gancho no build suportado. Validação automatizada; conferência visual dentro do jogo ainda pendente.

## 2.1.0 — 20/11/2026

- Whitelist opcional por SteamID64 em `guardiao.json`, relida a cada entrada. Desativada por padrão; ativa, restringe todos os jogadores, inclusive admins e no modo observar. Falhas na configuração recusam novas entradas e registram o motivo.
- Lista `admins`: o personagem recebido substitui a referência do servidor, sem reset nem kick de primeira entrada. A cópia anterior é preservada em `admin-historico/`; falhas de gravação recusam a entrada e tentam restaurar os arquivos anteriores.
- Arquivo de exemplo e documentação das duas listas. Listas começam vazias, sem conceder privilégios automaticamente. Testes automatizados de regras de acesso e persistência/backup de admins; entrada real no jogo ainda precisa ser conferida.

## 2.0.0 — 18/11/2026

- **Mudança de comportamento:** contas sem cópia no servidor recebem um personagem reiniciado em nível 1 e XP zero, preservando a aparência, com inventários vazios, sem habilidades compradas e apenas a calça inicial. O personagem recebido é preservado antes da troca; falhas impedem a entrada e são registradas.
- Mensagens globais após o reset: aos 5 segundos do login, `[Guardião] {nome do personagem} acabou de vir de outro mundo.`; 1 segundo depois, `[Guardiao] Por isto, {nome do personagem} vai precisar sair e reconectar novamente.`; após mais 5 segundos, solicitação de kick sem banimento, aguardando o envio completo do save.
- Consulta do nome real do personagem, com rótulo genérico quando indisponível, e cancelamento de temporizadores de conexões reutilizadas. Esses ganchos dependem do build confirmado do servidor.
- **Mudança de comportamento:** uma cópia de build anterior pode voltar a ser restaurada; a conversão depende de o jogo conseguir carregá-la e salvá-la. Mantida a exceção para cópias anteriores a períodos sem o Guardião.
- Leitura dos dados do Save Editor, relatórios JSON de entrada e servidor e geração automática de `personagem.html` estático por conta Steam, com ícones e imagens incorporados, equipamentos, mochila, habilidades e conhecimentos.
- Integração com o loader Shroudtopia e anúncio global de primeira entrada.
- Esta versão consolida as alterações locais que ainda exibiam 1.1.0. Validação: 14 testes automatizados e carregamento em instância isolada; sequência completa de reset, anúncios e kick ainda requer conferência com jogador conectado.
- A partir desta entrega, cada nova alteração entregue incrementará a versão conforme SemVer; recompilações idênticas mantêm a versão.

## 1.1.0 — 04/11/2026

### Atualizações do jogo, sem perder progresso

- **Funciona na atualização do jogo de novembro de 2026 (build 1285601)** sem recompilar: os ganchos são
  achados por padrão de bytes, como em outubro.
- **Nunca faz um jogador voltar no tempo.** Se os ganchos não puderem ser instalados, o guardião avisa e marca
  todas as cópias como desatualizadas; quando ele voltar a funcionar, renova a cópia de cada jogador em vez de
  restaurar uma antiga.
- **Percebe quando o servidor rodou sem ele.** Ele grava um sinal de vida a cada minuto
  (`guardiao-dados/ativo.json`); se o mundo foi gravado muito depois do último (DLL removida, ganchos não
  achados), as cópias são marcadas do mesmo jeito.
- **Uma cópia de outra versão do jogo nunca é usada para trocar um personagem.** Cada cópia agora guarda o
  build do servidor que a gravou; na primeira entrada depois de uma atualização do jogo, o personagem atual fica
  e a cópia é renovada. As cópias da 1.0.0 não têm build, então a primeira entrada divergente de cada jogador
  depois de atualizar é aceita uma vez.
- **Histórico:** as 10 últimas cópias de cada jogador ficam em `historico/`, prontas para serem postas de volta
  à mão.

### Um guardião que aparece

- **Mensagens no console para tudo:** carregamento, modo, jogadores conectando, a decisão de cada entrada,
  cópias guardadas, avisos e erros — com o tamanho do personagem antes e depois quando algo difere.
- **Letreiro de inicialização** mostrado quando o mundo termina de carregar, para não se perder entre as linhas
  de boot do jogo. Os endereços dos ganchos vão só para o `guardiao.log`.
- **BisectHosting:** o painel mostra o `DLLMods/loader.log`, mas não a saída do servidor; agora o guardião
  também acrescenta as linhas dele lá. A instalação na BisectHosting (enviar para `DLLMods/`) está documentada.
- **Inglês por padrão**, português com `"idioma": "pt-BR"` (ou `"language"`) no `guardiao.json`, que também
  aceita `{"mode": "observe"}` além de `{"modo": "observar"}`.
- **Quando não consegue se ligar ao servidor,** diz claramente que está DESLIGADO, que nada é alterado e como
  avisar.

### Aviso de versão nova

- Consulta o GitHub ao ligar e depois a cada 24 horas. Quando há versão nova, um aviso emoldurado aparece no
  console e se repete a cada hora; as consultas param, porque o guardião já sabe.
- Nunca baixa nada nem envia nada sobre o seu servidor. Desliga com `"checkUpdates": false`. É o único acesso à
  rede do guardião.

## 1.0.0 — 24/10/2026

Primeira versão: o servidor dedicado guarda a sua própria cópia do personagem e do conhecimento de cada
jogador e a põe de volta quando alguém entra com outra, com o save recusado guardado como prova, modo
observar, troca conferida e ganchos achados por padrão de bytes.
