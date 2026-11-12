# Plano de migração completa: Wails 3 + Solid

**Branch de planejamento:** `plan/wails3-solid-migration`  
**Estado:** implementação concluída nesta branch; validação local concluída, CI remoto pendente de execução após o push.
**Objetivo:** substituir o desktop Tauri 2/React e o host Native Messaging Rust por Wails 3/Solid e Go, mantendo o GNU `pass`/GPG/Git como fonte de verdade, o funcionamento no terminal e a extensão atual em Brave/Chrome.

## Resultado da migração

- Desktop em Solid/Vite e Wails 3.0.0, compilado com `-tags gtk3` e WebKit2GTK 4.1 para manter suporte ao Linux desta instalação e ao runner Ubuntu 22.04. `@wailsio/runtime` está fixado em 3.1.0; a ponte foi exercitada no aplicativo real.
- Núcleo Go em `internal/{vault,setup,gitvault,github,otp,pairing}` compartilhado pelo desktop e pelo host Go. Os fontes Rust, Tauri e React foram removidos.
- Protocolo Native Messaging v1, ID da extensão e nomes dos executáveis preservados. O teste de integração do host e os testes reais no Brave e Chrome passaram com perfis descartáveis.
- Instalação de release testada duas vezes com um servidor local e HOME descartável. Checksums, manifesto e estado de bloqueio foram preservados.
- A publicação de uma nova release permanece dependente da revisão da branch e do CI remoto. O instalador público continua apontando para a release da branch `main` até lá.

Medição local em 12/11/2026, com cache aquecido: build Go do desktop 2,343 s; verificação TypeScript e build Vite 2,773 s. O binário Wails ficou com 17.719.872 bytes; o binário Tauri instalado antes da migração tem 13.589.880 bytes. São medições pontuais neste computador, não uma comparação de builds limpos nem de memória em uso.

## 1. Escopo e decisões de arquitetura

- `src/desktop`: Solid + TypeScript + Vite no frontend; Wails 3 + Go no processo desktop.
- `src/native-host`: executável Go separado, com o mesmo nome instalado (`passage-native-host`) e o mesmo protocolo Native Messaging v1. Desktop e host importam um núcleo Go compartilhado, sem depender de uma janela aberta.
- `src/extension`: manter WXT/TypeScript e o ID publicado. Alterar apenas se um ajuste de protocolo for indispensável; qualquer mudança incompatível exige migração versionada e testes dos navegadores.
- `packages/core`: manter os contratos TypeScript úteis à extensão e à UI; gerar ou verificar tipos do bridge Go→TypeScript para evitar definições divergentes.
- GNU `pass`, GPG, `pinentry`, `gpg-agent`, Git e `gh` continuam responsáveis por seus respectivos dados/autenticação. Não reimplementar OpenPGP nem importar chaves privadas para o processo Passage.
- Preservar `PASSWORD_STORE_DIR`, `~/.password-store`, `XDG_CONFIG_HOME/passage/settings.json`, estado de pareamento, URLs de remotos, arquivos `.gpg` e `.gpg-id`. A migração não deve exigir reconectar navegador nem recriar cofre.
- Manter dois executáveis no pacote inicialmente. Isso preserva a instalação e impede que uma invocação do browser inicialize o runtime gráfico. Consolidar em um binário só poderá ser uma otimização posterior, medida e testada.
- Pin de uma versão estável exata de Wails 3 no `go.mod`, CLI e runtime npm; não usar `@latest` no CI. Verificar a versão e as dependências Linux no início da implementação.

## 2. Inventário atual e contratos a congelar

| Área | Fonte atual | Contrato a preservar |
| --- | --- | --- |
| Cofre | `src/desktop/src-tauri/src/vault.rs`, `setup.rs` | Escolha/criação/clone do cofre; validação de ID, caminho e symlinks; leitura/escrita por `pass`; hash do ciphertext para concorrência; parsing de usuário, URL e campos extras. |
| Git/GitHub | `git.rs`, `github.rs` | Commit apenas de `.gpg`/`.gpg-id`, recusa de texto claro, fast-forward, publicação inicial, backups e mesclagem que preserva as duas versões cifradas; autenticação delegada ao `gh`. |
| TOTP | `otp.rs` | Parsing de `otpauth://`, algoritmos/períodos/dígitos, vetores RFC 6238, segredo apenas em memória. |
| Pareamento | `pairing.rs`, `src/native-host/src/main.rs` | Código de aprovação, token por instalação e cofre, revogação, bloqueio, origem HTTPS, dupla verificação de autorização. |
| Extensão | `src/extension` | Mensagens v1, ID fixo, pareamento existente, escolha explícita antes de preencher, checagem da aba e da URL. |
| UI | `src/desktop/src/main.tsx`, `style.css` | Fluxos e estados atuais, limpeza de segredo em 30 s, identidade visual verde e layout compacto. |
| Distribuição | `scripts/*.sh`, `.github/workflows/release-linux.yml` | `~/.local/bin/passage`, host, extensão, manifestos Brave/Chrome, checksum, instalador sem clone e atualização sem apagar pareamentos. |

Antes de portar código, registrar fixtures **sem segredos reais** para formatos de entrada, estado JSON, mensagens do host e saída/erros esperados dos comandos. Conferir todos os comandos expostos em `src/desktop/src-tauri/src/lib.rs` e as mensagens em `src/native-host/src/main.rs`; a tabela acima é um resumo, não a lista exaustiva.

## 3. Etapas de execução

### Etapa 0 — Baseline e viabilidade Linux

1. Congelar o comportamento atual com cofre, chave, remoto Git e perfis de navegador descartáveis. Executar testes atuais e registrar falhas preexistentes.
2. Medir build limpo, rebuild do backend, build só do frontend, tempo de inicialização, tamanho do pacote e uso de memória do Tauri no mesmo hardware. Separar cache quente/frio e registrar versão de toolchain; a compilação de 6 min 30 s feita com cache novo é apenas um dado inicial.
3. Fazer um spike Wails 3 + Solid mínimo no Linux alvo, com chamada Go→UI, janela, `pinentry` e processo externo. Verificar `wails3 doctor`, GTK4/WebKitGTK 6.0, Wayland/X11 e empacotamento. Nesta máquina, `webkitgtk-6.0` não estava disponível em `pkg-config` em 12/11/2026; resolver isso ou definir explicitamente o piso de distro antes de aprovar o port.
4. Documentar matriz Linux suportada. O caminho GTK3 legado não deve sustentar a distribuição nova se a versão Wails escolhida já não o suportar. Rodar `passage` sem acesso à internet e confirmar que o frontend embutido abre.

**Gate:** protótipo Wails executa no Linux alvo e a lista de distros suportadas está explícita. Caso falhe, manter a branch como estudo e não substituir o app instalado.

### Etapa 1 — Núcleo Go sem UI

1. Criar um módulo Go interno (`internal/vault`, `internal/git`, `internal/otp`, `internal/pairing`, `internal/config` ou equivalente), com interfaces pequenas para subprocessos e filesystem. Evitar acoplamento ao Wails.
2. Portar descoberta e seleção de cofre, proteção contra symlinks e travessia de caminho, listagem, leitura, criação, edição e remoção. Usar `pass` e GPG como hoje. Preservar a ordem e o conteúdo das linhas não editadas.
3. Portar Git e GitHub CLI com as mesmas recusas, refs de backup e limites de arquivos. Nunca executar comandos por shell com texto interpolado; usar argumentos de processo.
4. Portar TOTP e pareamento/estado de bloqueio. Ler os dados persistidos pela versão Rust sem reformatar ou invalidar tokens existentes. Se houver mudança de esquema, fazer migração idempotente com backup e teste de rollback.
5. Testar paridade contra os fixtures e cofres descartáveis: erros de GPG, edição concorrente, symlink, arquivos extras no Git, remoto divergente, conflito `.gpg-id` e alteração de bloqueio durante a leitura.

**Gate:** o núcleo Go passa os testes de comportamento e segurança antes de ser exposto ao frontend ou à extensão.

### Etapa 2 — Host Native Messaging Go

1. Portar o framing Chromium (4 bytes little-endian + JSON), limites de mensagem, `requestId`, erros genéricos, timeouts e limpeza de processo. Reservar `stdout` exclusivamente para o protocolo; logs em `stderr` sem segredos.
2. Preservar o protocolo v1 e os comandos CLI existentes de administração (`list`, `list-json`, `lock-status`, `lock`, `unlock`, aprovação/revogação conforme inventário completo).
3. Reusar o núcleo Go para resolver cofre e entrada. Revalidar token, origem e bloqueio imediatamente antes de responder; preservar o vínculo do token ao cofre.
4. Testar diretamente o binário com `src/native-host/tests/integration.py`, adaptando apenas o caminho do executável. Depois testar Brave e Chrome reais em perfis descartáveis, incluindo navegação da mesma aba, revogação, bloqueio e instalação já pareada.

**Gate:** extensão existente funciona sem atualização obrigatória, sem novo pareamento e sem enfraquecer os controles documentados em `docs/security-review.md`.

### Etapa 3 — Desktop Wails 3 + Solid

1. Criar o shell Wails em `src/desktop`, registrar serviços Go com métodos explícitos e gerar bindings tipados. Não expor acesso arbitrário a filesystem, comando externo ou segredo pelo bridge.
2. Portar a UI React para componentes Solid por fluxo: cofre/configuração, lista e detalhe, criação/edição/exclusão, TOTP, Git/GitHub e extensões. Preservar o CSS/identidade visual atual, estados de carregamento e erros.
3. Implementar adaptador de serviços no frontend para reduzir acoplamento dos componentes aos bindings Wails; manter contratos TypeScript compartilháveis com a extensão quando fizer sentido.
4. Preservar a limpeza de segredos exibidos após 30 s, limpar estado ao trocar de cofre/entrada e ao bloquear extensão. Não afirmar que limpeza de estado apaga garantidamente memória do processo ou cache do `gpg-agent`.
5. Testar teclado, foco, rolagem em janela compacta, estados sem cofre/chave/Git e diálogos do `pinentry`. Garantir que o app não bloqueie a UI durante `pass`, Git ou `gh`.

**Gate:** todos os fluxos do desktop atual funcionam em Solid/Wails com os mesmos dados locais e sem regressões de segurança.

### Etapa 4 — Instalação, CI e transição

1. Atualizar `scripts/install-linux-user.sh`, `package-linux.sh`, workflow de release, caminhos de binário, ícone, `.desktop` e documentação. Manter os nomes instalados e o formato do arquivo de release enquanto possível.
2. Testar atualização sobre uma instalação Tauri/Rust com cofre e pareamentos descartáveis: instalar Wails/Go, iniciar app, ler/escrever, usar Brave/Chrome, atualizar novamente. Não apagar `~/.config/passage` nem o cofre.
3. Testar instalação sem repositório clonado por `curl -LsSf https://passage.plu.nz/install.sh | sh` contra release de teste. Conferir SHA-256, host registrado e extensão existente.
4. Medir novamente build limpo/rebuild, tamanho, startup e memória nas mesmas condições da etapa 0. Documentar o resultado real, inclusive caso Go não traga ganho relevante no Linux por causa de CGO/WebKit.
5. Remover Tauri/React/Rust e dependências só depois de todos os gates. Atualizar `README.md`, `docs/decisions.md`, plano de execução e revisão de segurança. Publicar release somente após CI e teste local de atualização passarem.

**Gate final:** o pacote novo substitui o antigo sem perda de cofre, autenticação GPG, tokens de pareamento ou compatibilidade do `pass` no terminal.

## 4. Segurança e compatibilidade: critérios de bloqueio

- Nenhum segredo, token de GitHub ou frase secreta GPG em logs, mensagens de erro, telemetria ou fixtures.
- Nenhuma mudança na identidade da extensão ou no manifesto Native Messaging que force pareamento novo sem um plano explícito.
- Nenhum `pass show` disparado apenas por listar entradas; descriptografar somente após seleção/ação do usuário.
- Nenhum acesso da página web visitada ao bridge Wails ou ao host além do protocolo da extensão e das verificações existentes.
- Preservar as garantias de caminho seguro, validação de entrada, hash de concorrência e limites Git. Falha de paridade bloqueia a retirada do binário Rust.
- Revisar a política de conteúdo e carregamento do WebView Wails; carregar apenas assets embutidos e endpoints de desenvolvimento em modo dev. Fazer revisão específica da superfície Go exposta ao JavaScript.

## 5. Sequência de entregas e estimativa para planejamento

| Entrega | Dependência | Tamanho relativo | Evidência de conclusão |
| --- | --- | --- | --- |
| Baseline e spike Linux | nenhuma | pequeno | matriz de dependências e benchmarks reproduzíveis |
| Núcleo Go do cofre/Git/TOTP | baseline | grande | testes de paridade e cofre descartável |
| Host Go | núcleo | médio | protocolo v1 e Brave/Chrome validados |
| Solid + Wails | núcleo | grande | fluxos desktop equivalentes |
| CI, instalador e release de teste | host + desktop | médio | upgrade Tauri→Wails e instalação sem clone |
| Limpeza e release estável | todos os gates | pequeno | documentação e pacote final verificados |

As estimativas são relativas; tempo de calendário depende sobretudo do port da lógica Rust (~3,5 mil linhas entre desktop e host), disponibilidade de WebKitGTK 6.0 nos alvos Linux e testes de paridade. A troca React→Solid é uma parte menor da migração total.

## 6. Fora deste plano

Cliente Android/iOS próprio, web pura, biometria, novos recursos do cofre e publicação da extensão em loja. A arquitetura Go não deve impedir esses trabalhos, mas Wails mobile ainda não é base estável assumida para eles. Terminal puro continua disponível via GNU `pass`; não é necessário substituir sua CLI.
