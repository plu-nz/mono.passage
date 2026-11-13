# Decisões do MVP para revisão

## Migração Wails 3 e Go

O desktop passou a usar Solid/Vite e Wails 3.0.0 com Go. O host Native Messaging também foi portado para Go e compartilha o núcleo em `internal/`. O backend GTK3/WebKit2GTK 4.1 permite compilar nesta máquina e no Ubuntu 22.04; GTK4/WebKitGTK 6.0 poderá ser adotado quando estiver disponível no ambiente alvo. O protocolo v1, os caminhos de configuração, o ID da extensão e os nomes dos executáveis foram mantidos. A UI usa serviços Go explícitos, sem ponte genérica para executar comandos externos. Esta seção substitui as referências históricas a Tauri, React e Rust abaixo.


Este registro separa o que já foi implementado das escolhas ainda provisórias. Ele pode ser alterado após o primeiro teste com um cofre de exemplo.

## 1. Estrutura do repositório

- `src/desktop`: interface Solid/Vite e shell Wails 3/Go.
- `src/extension`: extensão Chromium em WXT, para Brave e Chrome.
- `packages/core`: contratos TypeScript compartilhados.

`src` agrupa as aplicações executáveis conforme a preferência atual. Mantemos `packages` separado para bibliotecas reutilizáveis. Não há uma reimplementação do formato OpenPGP no MVP.

## 2. Leitura do cofre no Linux

**Implementado:** o desktop permite escolher um cofre existente ou criar um novo com uma chave GPG privada presente no sistema. Também oferece gerar uma chave pelo GPG e inicializar o cofre no mesmo fluxo; a frase secreta é coletada pelo `pinentry`, fora da interface Passage. A seleção fica em `XDG_CONFIG_HOME/passage/settings.json` (ou `~/.config/passage/settings.json`); `PASSWORD_STORE_DIR` tem prioridade, seguido da seleção e de `~/.password-store`. A criação chama `pass init` e recusa sobrescrever um cofre inicializado. O desktop lista os arquivos `.gpg` sem entrar em `.git` ou seguir links simbólicos e chama `pass show` para abrir uma entrada. O conteúdo completo só é solicitado quando o usuário seleciona a entrada. A primeira linha é tratada como senha; as demais ficam em “Outros campos”. A interface limpa o conteúdo exibido após 30 segundos. O núcleo Go interpreta `login`/`username` e `url`; a UI cria e edita entradas por `pass` e preserva as outras linhas. Atualização e remoção exigem hash do arquivo cifrado lido anteriormente, mas há uma janela de corrida com alterações externas entre a verificação e o subprocesso `pass`.

**Motivo:** delegar descriptografia e regras do cofre ao GNU `pass` e ao GPG preserva a interoperabilidade com o terminal e com Android Password Store. Nenhum segredo é gravado pelo aplicativo nesta etapa.

**Limite importante:** encontrar o diretório não significa que a chave esteja desbloqueada. `pass show` pode solicitar a senha via `pinentry` e o `gpg-agent` pode mantê-la em cache depois que a interface limpa o texto. O estado `available` significa apenas “cofre presente”, nunca “chave desbloqueada”. O processo da interface também pode manter dados sensíveis em memória até a coleta de lixo; uma sessão curta de visualização não equivale a apagar memória com garantia.

**Recuperação:** a chave privada não é salva no Git nem exportada automaticamente pelo Passage. Depois da geração, a interface mostra a impressão digital e orienta fazer backup da chave e da frase secreta. Se a inicialização do cofre falhar após o GPG criar a chave, a chave permanece no chaveiro local; o erro informa como retomar sem gerar outra.

**Validade da chave:** a chave gerada não expira automaticamente. Sem um fluxo de renovação na interface, uma data de expiração interromperia a criação de novas entradas no `pass` sem aviso. Isso não substitui rotação quando necessária; o usuário pode ajustar a validade pelo GPG.

## 3. Conector dos navegadores

**Protótipo implementado:** host Native Messaging com pareamento por código aprovado no desktop ou terminal, token por instalação vinculado ao diretório do cofre, revogação pela interface ou CLI e leitura limitada à origem HTTPS associada a um campo `url` da entrada. Trocar de cofre exige novo pareamento. A extensão consulta o host e preenche após escolha do usuário. O estado do GPG é compartilhado pelo sistema. Testes de integração diretos passaram. Em perfis descartáveis de Brave Origin e Chrome estável, o popup real pareou, listou e preencheu formulários HTTPS com o manifesto original; também recusou o preenchimento após navegação da mesma aba para outra URL. A matriz incluiu login comum, etapas separadas de usuário e senha, `autocomplete=off`, campos ambíguos, senha invisível, cadastro com nova senha, iframe e página sem login. O formulário real de login do GitHub passou nos dois navegadores com dados fictícios, sem submissão. O Chrome estável foi testado em janela normal com carregamento pelo protocolo de depuração, pois a flag `--load-extension` foi removida. Ainda falta validar outros sites e a instalação manual em perfis já usados.

**Identidade e instalação:** o manifesto agora inclui uma chave pública fixa e produz o ID `edmjpcolampgjbnmldchmnkinmhakjgm` em Brave Origin e Chrome estável, independentemente da pasta temporária de teste. A chave privada correspondente fica fora do repositório. O pacote inclui o ID e valida que ele corresponde à chave pública compilada; o instalador registra automaticamente esse ID nos manifestos de Brave e Chrome. A instalação pública de `v0.2.1` verificou os dois manifestos e a extensão instalada sem checkout. Executar o instalador duas vezes preservou o arquivo de pareamentos e o ID. A extensão ainda precisa ser carregada manualmente uma vez em cada navegador, e cada instalação deve ser pareada. Publicação em loja pode exigir uma estratégia própria para manter o ID.

Para preencher, a extensão precisará solicitar acesso para o domínio da aba atual e só injetar credenciais após uma ação do usuário. O protocolo deve associar a resposta à aba e à origem que iniciou o pedido; nunca enviar segredos a uma página arbitrária nem aceitar um domínio fornecido pela própria página.

## Sincronização Git na branch `main`

O desktop mostra o estado do repositório do cofre e oferece baixar/enviar quando há branch com upstream. `pull` exige árvore limpa, busca alterações e só aceita avanço direto. `push` busca o remoto antes de enviar e recusa se ele avançou. **Registrar alterações** cria commits só com `.gpg` e `.gpg-id`, recusa outros arquivos e exige pacotes OpenPGP cifrados nos `.gpg`. Divergência pode ser resolvida por ação explícita que mantém a versão local de `.gpg` conflitante e salva a remota como outra entrada cifrada, sem mesclar ciphertext como texto. Conflitos em `.gpg-id`, remoções e renomeações exigem Git manual. Antes de mesclar, o Passage guarda os dois commits em `refs/passage/backup/`; um erro aborta a mesclagem. Há uma janela de concorrência se outro processo modificar o cofre durante fetch, merge ou push. O ciclo publicar, clonar, enviar e baixar foi validado com um repositório privado temporário no GitHub e dados fictícios.

### Autenticação GitHub

O desktop usa o GitHub CLI (`gh`) para autenticação pelo navegador. O Passage exibe apenas a conta ativa, abre o fluxo do CLI no terminal padrão e não recebe o token. Para um cofre que já seja um repositório Git, configura o helper de credenciais de `github.com` somente no `.git/config` local; não altera remotos nem cria repositórios. O armazenamento do token segue o comportamento do `gh`, incluindo seu fallback em arquivo local quando o keyring não funciona. A integração não substitui autenticação SSH ou de outros servidores Git.

O desktop pode inicializar Git no diretório do cofre e adicionar `origin` quando ele não existe. Para um remoto vazio, **Publicar cofre novo** cria o primeiro commit com `.gpg-id` e `.gpg`, recusa arquivos extras ou já preparados no índice e configura o upstream sem `force`. Se já houver um commit local limpo, publica esse commit; portanto o conteúdo de commits criados fora do Passage continua sob responsabilidade do usuário. Um remoto com histórico exige ligação manual. Testes com remotos descartáveis cobrem a publicação inicial, a recusa de arquivo em texto claro e a divergência entre clones. Um repositório privado temporário no GitHub também foi usado para validar publicação, clone, push e pull com arquivos fictícios. A remoção desse repositório exige o escopo `delete_repo`, ausente na sessão `gh` atual.

**Clonar cofre existente** usa `git clone` para um diretório inexistente, exige branch `main` e valida `.gpg-id` antes de selecioná-lo. A chave privada GPG não vem do Git e deve ser importada separadamente. Uma falha após o clone deixa o diretório criado para inspeção, sem selecionar o cofre. A autenticação usa os helpers Git já disponíveis no sistema; a integração com `gh` ainda depende da configuração Git local/global usada no ambiente.

### Bloqueio e TOTP

O host mantém um estado `locked` persistente. A interface pode bloquear/liberar o acesso das extensões; o host verifica o estado antes e após a leitura. Isso não bloqueia `pass` no terminal nem limpa o cache do `gpg-agent`. Há uma janela residual para um pedido já em andamento entre a última verificação e o envio da resposta; por isso a interface descreve o controle como bloqueio do acesso das extensões, não como bloqueio criptográfico do cofre.

O desktop lê URI `otpauth://totp/...` em linha própria, como no `pass-otp`, e `otpauth: URI`. Gera código somente em memória e valida algoritmo, dígitos e período. Os testes incluem os vetores da RFC 6238 para SHA-1, SHA-256 e SHA-512. Não há geração de TOTP pela extensão neste estágio.

## 4. Próximas etapas recomendadas

1. Exercitar leitura e escrita com cofre GPG descartável, incluindo `PASSWORD_STORE_DIR`, falha de GPG e alteração externa pelo terminal.
2. Validar instalação manual em perfis já usados de Brave e Chrome e mais sites reais.
3. Reduzir a janela de corrida da edição antes de recomendar dados reais.
4. Acompanhar uso da release `v0.2.1` e repetir os testes após mudanças no protocolo do host ou nas permissões da extensão.
5. Projetar biometria e bloqueio considerando a chave GPG e o cache do `gpg-agent`, antes de prometer desbloqueio pela digital no primeiro acesso.

## 5. Decisões ainda abertas

- Canal de publicação em loja e como preservar o ID já fixado na extensão descompactada.
- Como autenticar o host local contra o desktop e armazenar autorizações por instalação.
- Convenção de metadados de URL e usuário dentro de entradas `pass`.
- Política de edição concorrente e resolução de conflitos Git.
- Desenho de desbloqueio biométrico que proteja efetivamente a chave privada.
- Futuro cliente web sem conector e eventual app Android próprio.

## 6. Endereço do instalador

O código público fica em `github.com/plu-nz/passage`. A raiz `https://passage.plu.nz/` publica a landing page de `website/index.html` por meio do Cloudflare Worker `passage-landing`, associado ao subdomínio como Worker Custom Domain. O endereço `https://passage.plu.nz/install.sh` continua sendo um redirecionamento HTTP 302 da Cloudflare para `https://raw.githubusercontent.com/plu-nz/passage/main/scripts/install.sh`, por uma Single Redirect Rule na fase `http_request_dynamic_redirect`. O instalador continua versionado neste repositório; o atalho não hospeda uma segunda cópia.

Para reconstruir o Worker, execute `node website/build-worker.mjs`; o resultado fica em `website/dist/worker.js`. O deploy atual é manual pela API da Cloudflare. Mudanças na página exigem gerar e publicar novamente o Worker; um push no GitHub, por si só, não atualiza o site. O Worker também implementa o redirecionamento `/install.sh` como fallback caso a regra da Cloudflare seja removida.

O comando curto executa o script da branch `main`. O script baixa o pacote Linux x86_64 do release mais recente, verifica seu SHA-256 e instala sem clonar ou compilar o repositório. O arquivo de checksum protege contra transferência corrompida, mas ambos vêm do mesmo release. Versões fixas e assinatura de artefatos podem ser adicionadas antes de uma distribuição estável.
