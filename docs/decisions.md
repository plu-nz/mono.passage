# Decisões do MVP para revisão

Este registro separa o que já foi implementado das escolhas ainda provisórias. Ele pode ser alterado após o primeiro teste com um cofre de exemplo.

## 1. Estrutura do repositório

- `src/desktop`: interface React/Vite e shell Tauri 2.
- `src/extension`: extensão Chromium em WXT, para Brave e Chrome.
- `packages/core`: contratos TypeScript compartilhados.

`src` agrupa as aplicações executáveis conforme a preferência atual. Mantemos `packages` separado para bibliotecas reutilizáveis. Não há uma reimplementação do formato OpenPGP no MVP.

## 2. Leitura do cofre no Linux

**Implementado:** o desktop usa `PASSWORD_STORE_DIR` ou `~/.password-store`, lista os arquivos `.gpg` sem entrar em `.git` ou seguir links simbólicos e chama `pass show` para abrir uma entrada. O conteúdo completo só é solicitado quando o usuário seleciona a entrada. A primeira linha é tratada como senha; as demais ficam em “Outros campos”. A interface limpa o conteúdo exibido após 30 segundos. O núcleo Rust agora também interpreta `login`/`username` e `url` e oferece operações de escrita por `pass`; a UI ainda não as expõe. Atualização e remoção exigem hash do arquivo cifrado lido anteriormente, mas há uma janela de corrida com alterações externas entre a verificação e o subprocesso `pass`.

**Motivo:** delegar descriptografia e regras do cofre ao GNU `pass` e ao GPG preserva a interoperabilidade com o terminal e com Android Password Store. Nenhum segredo é gravado pelo aplicativo nesta etapa.

**Limite importante:** encontrar o diretório não significa que a chave esteja desbloqueada. `pass show` pode solicitar a senha via `pinentry` e o `gpg-agent` pode mantê-la em cache depois que a interface limpa o texto. O estado `available` significa apenas “cofre presente”, nunca “chave desbloqueada”. O processo da interface também pode manter dados sensíveis em memória até a coleta de lixo; uma sessão curta de visualização não equivale a apagar memória com garantia.

## 3. Conector dos navegadores

**Protótipo implementado:** host Native Messaging com pareamento por código aprovado no terminal, token por instalação, revogação por CLI e leitura limitada à origem HTTPS associada a um campo `url` da entrada. A extensão consulta o host e preenche após escolha do usuário. O estado do GPG é compartilhado pelo sistema. Ainda faltam validação ponta a ponta em Brave/Chrome, aprovação e revogação pela UI desktop, distribuição automática do host e auditoria de segurança antes de considerar o fluxo concluído.

Para preencher, a extensão precisará solicitar acesso para o domínio da aba atual e só injetar credenciais após uma ação do usuário. O protocolo deve associar a resposta à aba e à origem que iniciou o pedido; nunca enviar segredos a uma página arbitrária nem aceitar um domínio fornecido pela própria página.

## 4. Próximas etapas recomendadas

1. Criar cofre de teste descartável e exercitar leitura no desktop, incluindo `PASSWORD_STORE_DIR`, falha de GPG e atualização após uma alteração pelo terminal.
2. Implementar host Native Messaging e instalação local para Brave/Chrome com IDs de extensão fixos por distribuição.
3. Implementar pareamento, revogação e pedidos de preenchimento autorizados pelo desktop, com teste em ambos os navegadores.
4. Adicionar edição compatível com `pass` e sincronização Git com tratamento explícito de conflitos.
5. Projetar biometria e bloqueio considerando a chave GPG e o cache do `gpg-agent`, antes de prometer desbloqueio pela digital no primeiro acesso.

## 5. Decisões ainda abertas

- Como distribuir e manter IDs de extensão estáveis em Brave e Chrome.
- Como autenticar o host local contra o desktop e armazenar autorizações por instalação.
- Convenção de metadados de URL e usuário dentro de entradas `pass`.
- Política de edição concorrente e resolução de conflitos Git.
- Desenho de desbloqueio biométrico que proteja efetivamente a chave privada.
- Futuro cliente web sem conector e eventual app Android próprio.

## 6. Endereço do instalador

O código público fica em `github.com/plu-nz/passage`. A raiz `https://passage.plu.nz/` publica a landing page de `website/index.html` por meio do Cloudflare Worker `passage-landing`, associado ao subdomínio como Worker Custom Domain. O endereço `https://passage.plu.nz/install.sh` continua sendo um redirecionamento HTTP 302 da Cloudflare para `https://raw.githubusercontent.com/plu-nz/passage/main/scripts/install.sh`, por uma Single Redirect Rule na fase `http_request_dynamic_redirect`. O instalador continua versionado neste repositório; o atalho não hospeda uma segunda cópia.

Para reconstruir o Worker, execute `node website/build-worker.mjs`; o resultado fica em `website/dist/worker.js`. O deploy atual é manual pela API da Cloudflare. Mudanças na página exigem gerar e publicar novamente o Worker; um push no GitHub, por si só, não atualiza o site. O Worker também implementa o redirecionamento `/install.sh` como fallback caso a regra da Cloudflare seja removida.

O comando curto executa o script da branch `main`. O script baixa o pacote Linux x86_64 do release mais recente, verifica seu SHA-256 e instala sem clonar ou compilar o repositório. O arquivo de checksum protege contra transferência corrompida, mas ambos vêm do mesmo release. Versões fixas e assinatura de artefatos podem ser adicionadas antes de uma distribuição estável.
