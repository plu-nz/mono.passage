# Conector local (protótipo)

O host Native Messaging atende Brave e Chrome usando o cofre GNU `pass` do usuário. Ele é iniciado pelo navegador sob demanda, sem exigir que a janela Tauri esteja aberta. A aprovação por CLI é provisória até existir uma tela de pareamento no desktop.

O instalador de release instala `passage-native-host` junto ao desktop. Para registrar os navegadores, informe os IDs reais das extensões após carregá-las:

```sh
passage-install-native-host "$HOME/.local/bin/passage-native-host" ID_BRAVE ID_CHROME
```

Também é possível definir `PASSAGE_BRAVE_EXTENSION_ID` e `PASSAGE_CHROME_EXTENSION_ID` antes de executar o instalador, para registrar ambos automaticamente. Releases antigos, sem host no arquivo, continuam instaláveis; não fornecem conexão com navegadores.

## Desenvolver e registrar

```sh
cargo build --manifest-path src/native-host/Cargo.toml
sh scripts/install-native-host.sh \
  "$PWD/src/native-host/target/debug/passage-native-host" \
  ID_MOSTRADO_EM_BRAVE_EXTENSIONS \
  ID_MOSTRADO_EM_CHROME_EXTENSIONS
```

Para usar outro `CARGO_TARGET_DIR`, passe o caminho real do binário produzido. Os IDs de extensão aparecem no gerenciador de cada navegador após carregar a extensão descompactada. O script registra manifestos para Brave e Chrome estáveis; também registra perfis Brave Beta, Nightly e Brave Origin já existentes. É preciso Python 3.

## Parear e revogar

O popup solicita pareamento e mostra um código de 12 caracteres que expira em 5 minutos. Confirme localmente:

```sh
passage-native-host approve CODIGO
passage-native-host list
passage-native-host revoke chrome-extension://ID_DA_EXTENSAO/ ID_DA_INSTALACAO
```

`list` mostra a origem e o ID da instalação; `revoke` bloqueia pedidos seguintes. O token emitido após a aprovação é mostrado uma vez ao navegador e armazenado apenas como hash no host. O estado local fica em `~/.config/passage/native-host.json` (ou `XDG_CONFIG_HOME/passage`) com permissões de usuário.

Cada pareamento é válido apenas para o cofre selecionado quando foi aprovado. Ao escolher outro cofre no desktop, a extensão precisa de novo pareamento para acessá-lo. A lista na interface desktop mostra somente os pareamentos do cofre atual; `list` no CLI mostra todos para permitir revogação. Se você voltar a selecionar um cofre anterior, o pareamento antigo desse cofre continua válido até ser revogado.

O desktop pode bloquear o acesso das extensões com `passage-native-host lock` e liberar com `passage-native-host unlock`. O estado persiste após fechar o aplicativo ou reiniciar o computador; `passage-native-host lock-status` o consulta. O bloqueio corta pedidos novos mesmo quando o `gpg-agent` ainda tem a chave em cache. Ele não bloqueia o uso direto de `pass` no terminal nem apaga uma senha que já esteja visível numa página do navegador.

## Limites atuais

- A extensão precisa usar a origem da aba obtida pela API do navegador e validar a aba novamente antes de preencher. O host exige origem HTTPS e correspondência exata com um campo `url` da entrada.
- O host decripta os metadados das entradas para encontrar correspondências; `pass`/GPG podem solicitar a senha da chave neste momento. Um cofre grande pode tornar a busca lenta.
- A origem da extensão é informada pelo navegador como argumento do processo. Os manifestos limitam os IDs aceitos pelo navegador; outro processo local do mesmo usuário pode invocar o executável diretamente e, por isso, token e aprovação continuam necessários.
- O protocolo e o instalador ainda não foram submetidos à matriz final de testes Brave/Chrome. Use um cofre descartável nesta fase.
