# Decisões do MVP para revisão

Este registro separa o que já foi implementado das escolhas ainda provisórias. Ele pode ser alterado após o primeiro teste com um cofre de exemplo.

## 1. Estrutura do repositório

- `src/desktop`: interface React/Vite e shell Tauri 2.
- `src/extension`: extensão Chromium em WXT, para Brave e Chrome.
- `packages/core`: contratos TypeScript compartilhados.

`src` agrupa as aplicações executáveis conforme a preferência atual. Mantemos `packages` separado para bibliotecas reutilizáveis. Não há uma reimplementação do formato OpenPGP no MVP.

## 2. Leitura do cofre no Linux

**Implementado:** o desktop usa `PASSWORD_STORE_DIR` ou `~/.password-store`, lista os arquivos `.gpg` sem entrar em `.git` ou seguir links simbólicos e chama `pass show` para abrir uma entrada. O conteúdo completo só é solicitado quando o usuário seleciona a entrada. A primeira linha é tratada como senha; as demais ficam em “Outros campos”. A interface limpa o conteúdo exibido após 30 segundos.

**Motivo:** delegar descriptografia e regras do cofre ao GNU `pass` e ao GPG preserva a interoperabilidade com o terminal e com Android Password Store. Nenhum segredo é gravado pelo aplicativo nesta etapa.

**Limite importante:** encontrar o diretório não significa que a chave esteja desbloqueada. `pass show` pode solicitar a senha via `pinentry` e o `gpg-agent` pode mantê-la em cache depois que a interface limpa o texto. O estado `available` significa apenas “cofre presente”, nunca “chave desbloqueada”. O processo da interface também pode manter dados sensíveis em memória até a coleta de lixo; uma sessão curta de visualização não equivale a apagar memória com garantia.

## 3. Conector dos navegadores

**Ainda pendente:** a extensão só testa um `ping` de Native Messaging. Não expomos `list` ou `read` pelo host antes de implementar pareamento por instalação, revogação e uma política explícita de acesso por origem. O desktop deve ser a autoridade de autorização das extensões. O estado do GPG será compartilhado pelo sistema, mas cada extensão terá uma identidade e permissões próprias.

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

O código público fica em `github.com/plu-nz/passage`. O endereço `https://passage.plu.nz/install.sh` é um redirecionamento HTTP 302 da Cloudflare para `https://raw.githubusercontent.com/plu-nz/passage/main/scripts/install.sh`. A raiz `https://passage.plu.nz/` redireciona para o repositório. O subdomínio usa um registro DNS com proxy da Cloudflare e duas Single Redirect Rules na fase `http_request_dynamic_redirect`. O instalador continua versionado neste repositório; o atalho não hospeda uma segunda cópia.

O comando curto executa o script da branch `main`, portanto acompanha as atualizações dessa branch. Antes de uma distribuição estável, devemos oferecer versões fixas e binários de release para evitar recompilar todas as dependências em cada instalação.
