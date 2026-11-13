# Passage

Passage é uma proposta de interface multiplataforma para cofres compatíveis com o [GNU `pass`](https://www.passwordstore.org/). O cofre pertence ao usuário: seus arquivos cifrados são sincronizados por um repositório Git privado que pode ser hospedado em um serviço Git ou em infraestrutura própria.

Site do projeto: [passage.plu.nz](https://passage.plu.nz/).

Este documento registra os requisitos discutidos até agora. Ele descreve o produto desejado, não afirma que as funcionalidades já foram implementadas.

## Estado do código

O app desktop permite escolher um cofre GNU pass existente, criar outro com uma chave GPG nova ou já disponível, listar, buscar, abrir, criar, editar e excluir entradas, gerar TOTP e bloquear o acesso das extensões. O aplicativo também contém conexão à conta GitHub pelo GitHub CLI, inicialização do Git no cofre, configuração do remoto, registro de alterações cifradas, sincronização Git e uma ação para preservar as duas versões de entradas `.gpg` conflitantes. Aprovação e revogação de extensões estão na interface, e o host Native Messaging oferece busca por URL e preenchimento após ação do usuário. A matriz de formulários controlados passou em Brave Origin e Chrome estável; o formulário real de login do GitHub também foi testado nos dois navegadores com credenciais fictícias, sem envio do formulário.

| Diretório | Responsabilidade |
| --- | --- |
| `core` | Tipos e contrato compartilhado do cofre e das mensagens locais. |
| `desktop` | Interface Solid/Vite e aplicativo Wails 3/Go para Linux. |
| `extension` | Extensão Manifest V3 para Brave e Chrome, construída com WXT. |
| `native-host` | Conector local Native Messaging para as extensões. |
| `web` | Landing page estática e gerador do Worker que a publica. |

Para desenvolver no Linux, instale Node.js, pnpm, Go 1.26+, GNU `pass`, GPG, GTK3 e WebKit2GTK 4.1. O desktop usa Wails 3.0.0 com o backend GTK3 nesta fase; veja as [dependências Linux do Wails](https://v3.wails.io/getting-started/installation/). Na raiz do repositório:

```bash
pnpm install
pnpm check
pnpm --filter @passage/desktop build
go run -tags gtk3 ./desktop
```

Para gerar a extensão, execute `pnpm build:extension` e carregue `extension/.output/chrome-mv3` como extensão descompactada em `brave://extensions` ou `chrome://extensions`. Siga [as instruções do conector local](native-host/README.md) para compilá-lo e registrar os IDs das extensões; o código mostrado pelo popup pode ser aprovado na interface desktop ou pelo CLI. Para usar o desktop, instale GNU `pass` e GPG. A chave GPG e o `gpg-agent` continuam sob controle do sistema.

O teste de navegador usa um cofre, servidor HTTPS e perfis descartáveis, com o manifesto original da extensão. Após compilar extensão e host, execute `node extension/tests/browser-smoke.mjs CAMINHO_DO_NAVEGADOR CAMINHO_DO_HOST CAMINHO_DO_INSTALADOR_DO_HOST CENÁRIO`. Os cenários são `standard`, `usernameOnly`, `passwordOnly`, `autocompleteOff`, `ambiguousPassword`, `ambiguousUsername`, `hiddenPassword`, `transparentPassword`, `newPassword`, `iframe`, `noFields`, `githubLive` e `herokuLive`. Cada cenário verifica pareamento, lista, resultado do preenchimento e recusa após navegação. Os cenários `*Live` preenchem apenas campos de teste em páginas externas; não enviam formulários. `githubLive` passou no Brave Origin e Chrome estável; `herokuLive` passou no Chrome estável, mas o Brave não carregou essa página neste ambiente. O Chrome estável recente não aceita `--load-extension`; execute o teste com `PASSAGE_BROWSER_HEADED=1` para carregar a extensão no perfil descartável pelo protocolo de depuração. A matriz passou no Chrome estável e no Brave Origin; o teste exige OpenSSL.

### Abrir ou criar um cofre no desktop

No painel **Cofre**, informe o caminho absoluto do diretório e escolha **Abrir existente** para um cofre já inicializado com `.gpg-id`. Para criar, escolha uma chave GPG privada na lista e clique em **Criar cofre**, ou abra **Criar chave GPG e cofre** para gerar uma chave nova pelo GPG e inicializar o cofre em sequência. A frase secreta é solicitada pelo `pinentry` do sistema; o Passage não a recebe. O Passage usa `pass init` para criar a estrutura compatível com GNU pass. Para abrir um cofre de outro dispositivo, importe a chave privada correspondente antes de tentar ler as entradas.

Depois de criar uma chave nova, guarde uma cópia segura da chave privada e a frase secreta. O Git sincroniza apenas os arquivos cifrados do cofre; perder a chave privada impede recuperar as senhas. O Passage mostra a impressão digital da chave criada e não exporta automaticamente material secreto. A criação da chave é uma operação separada da inicialização do cofre: se a segunda etapa falhar, a chave gerada permanece no GPG e pode ser usada para tentar criar o cofre novamente.

Para fazer backup, substitua `FINGERPRINT` pela impressão digital mostrada no aplicativo e escolha um destino seguro fora do repositório Git:

```sh
umask 077
gpg --armor --export-secret-keys FINGERPRINT > /caminho/seguro/passage-secret-key.asc
```

Para restaurar em outro dispositivo, copie o arquivo de backup por um meio seguro e execute `gpg --import /caminho/seguro/passage-secret-key.asc`. Depois confirme a chave com `gpg --list-secret-keys --fingerprint` e teste a leitura de uma entrada com `PASSWORD_STORE_DIR=/caminho/do/cofre pass show ENTRADA`. A exportação, importação e leitura foram verificadas com uma chave e um cofre descartáveis. Guarde o backup separadamente do cofre Git.

O diretório escolhido fica salvo na configuração local do Passage e é usado pelo desktop e pelo conector das extensões. `PASSWORD_STORE_DIR`, quando definido no ambiente do processo, tem prioridade e impede trocar o diretório pela interface. No terminal, para usar um cofre fora de `~/.password-store`, defina a mesma variável, por exemplo `PASSWORD_STORE_DIR=/caminho/do/cofre pass ls`.

### Criar e editar entradas

Com o cofre aberto, clique em **Nova entrada** e informe o caminho da entrada, a senha, o usuário e a URL. O Passage grava pelo GNU `pass`: a senha fica na primeira linha, seguida por `login:` e `url:`. O campo **Outras linhas do pass** aceita metadados adicionais, como `otpauth:` ou anotações. Para editar uma entrada, abra-a e clique em **Editar**. Para removê-la, clique em **Excluir** e confirme. Essas operações verificam se o arquivo cifrado mudou desde a leitura; nesse caso, atualize a entrada antes de tentar novamente. A senha aberta é removida da interface após 30 segundos; o formulário de criação fecha após 30 segundos sem alteração da senha. A operação não faz `git push` automaticamente.

### Conectar uma conta GitHub

Instale o [GitHub CLI (`gh`)](https://cli.github.com/). No painel **Sincronização Git**, clique em **Entrar com GitHub**. O Passage abre `gh auth login` no terminal padrão; conclua o login no navegador e volte ao Passage para clicar em **Verificar conta**. Se o terminal padrão não puder ser aberto, execute `gh auth login --web` manualmente. O token é administrado pelo `gh`, que pode usar o armazenamento seguro do sistema ou, se ele não estiver disponível, um arquivo local; verifique `gh auth status` para conhecer o local usado.

Se o cofre já for a raiz de um repositório Git, **Usar neste cofre** configura apenas o `.git/config` desse cofre para pedir ao `gh` as credenciais de remotos HTTPS em `github.com`. O Passage não recebe nem salva o token. Essa etapa não cria um repositório remoto nem altera o remoto existente; configure a URL e o branch de acompanhamento pelo Git ou `pass git`. Remotos SSH e servidores Git privados continuam usando sua autenticação normal.

No mesmo painel, **Iniciar Git neste cofre** cria um repositório local na branch `main` se o diretório ainda não tiver `.git`. **Adicionar remoto origin** registra a URL informada sem substituir um remoto existente. Crie antes um repositório Git privado vazio. **Publicar cofre novo** verifica que o remoto não tem histórico, cria o primeiro commit somente com `.gpg-id` e arquivos `.gpg` (se ainda não houver commit) e configura o upstream. A ação recusa arquivos extras, links simbólicos e arquivos já preparados no Git; nunca força o envio. Se o remoto já tiver histórico, resolva essa ligação pelo Git antes de sincronizar pelo Passage. A chave privada GPG continua fora do remoto. Alterações futuras podem ser registradas com `pass git` no terminal.

Para recuperar um cofre já publicado em outra instalação, informe um diretório que ainda não existe e a URL do repositório no painel **Cofre**, então clique em **Clonar cofre existente**. O remoto precisa ter a branch `main` e `.gpg-id` na raiz. O Passage seleciona o cofre após o clone; para abrir as entradas, importe separadamente a chave GPG privada correspondente. O clone usa a autenticação Git já disponível no sistema e não modifica um diretório existente.

Após criar, editar ou excluir entradas, **Registrar alterações** cria um commit somente com arquivos `.gpg` e `.gpg-id`; arquivos fora desse formato, links simbólicos ou `.gpg` sem pacotes OpenPGP cifrados fazem a operação parar. **Baixar** e **Enviar** permanecem ações separadas. Se os históricos divergirem, **Preservar ambas as versões** mescla os commits e mantém a versão local de cada `.gpg` conflitante no nome original, salvando a versão remota como `nome.remote-HASH.gpg`. Revise essas entradas depois da mesclagem e envie quando estiver satisfeito. Conflitos em `.gpg-id`, remoções concorrentes e renomeações exigem resolução manual pelo Git. O Passage cria refs de backup em `refs/passage/backup/` antes da tentativa de mesclagem.

### TOTP e bloqueio das extensões

Para gerar um código, armazene um URI `otpauth://totp/...` em uma linha da entrada, como faz o `pass-otp`, abra a entrada e clique em **Gerar código TOTP**. O Passage também lê uma linha `otpauth: URI`. O código fica em memória e some ao expirar ou após fechar a entrada; ele não é gravado no cofre.

No painel **Extensões autorizadas**, **Bloquear extensões** impede novas leituras pelo host Native Messaging, inclusive se a chave GPG estiver em cache. **Liberar extensões** restaura esse acesso. O estado persiste até outra ação; bloquear não fecha o cache do `gpg-agent` e não impede `pass show` no terminal. Um pedido já em andamento pode terminar antes de o bloqueio ser aplicado.

## Instalação no Linux

Em Linux x86_64, com `curl`, `tar` e `sha256sum`:

```sh
curl -LsSf https://passage.plu.nz/install.sh | sh
```

O script baixa o binário pronto do [release mais recente](https://github.com/plu-nz/passage/releases/latest), verifica o SHA-256 e instala `~/.local/bin/passage` com um atalho no menu. A partir de `v0.2.1`, o pacote instala a extensão descompactada em `~/.local/share/passage/extension-chromium` (ou em `XDG_DATA_HOME/passage/extension-chromium`) e registra automaticamente o host para Brave e Chrome. Carregue essa pasta uma vez em `brave://extensions` e `chrome://extensions`, com o modo de desenvolvedor ativado, e aprove cada navegador no painel **Extensões autorizadas**. O ID da extensão descompactada é fixo: `edmjpcolampgjbnmldchmnkinmhakjgm`. Não é preciso clonar o repositório nem instalar Node.js, pnpm ou Go para usar o release. GNU `pass`, GPG, Python 3, GTK3 e WebKit2GTK 4.1 são dependências de execução. `~/.local/bin` deve estar no `PATH` da sessão.

O mesmo comando atualiza uma instalação existente para a release mais recente. Feche e abra o Passage após atualizar e clique em **Recarregar** na extensão em `brave://extensions` e `chrome://extensions` para aplicar a versão nova. A versão instalada muda apenas quando uma nova release é publicada.

A chave pública da identidade da extensão está em `extension/identity.json`. A chave privada correspondente foi gerada fora do repositório, em `~/.local/share/passage/signing/extension.pem`, com permissão de acesso somente ao usuário. Faça backup **cifrado** dela fora do cofre Git; não a inclua no repositório nem no pacote. Ela será necessária se uma distribuição CRX assinada com o mesmo ID for adotada. A extensão descompactada funciona apenas com a chave pública do manifesto. Uma futura publicação em loja precisa confirmar como preservar o mesmo ID antes da migração.

Para publicar uma nova versão Linux a partir de `main`, execute com acesso de escrita ao repositório:

```sh
gh workflow run release-linux.yml --ref main -f version=0.2.2
gh run watch
```

O [workflow manual](.github/workflows/release-linux.yml) valida que a tag ainda não existe, compila desktop, conector e extensão em Ubuntu 22.04, executa as verificações e publica os arquivos e seus SHA-256 como release mais recente. A versão é informada sem o prefixo `v` e precisa ser incrementada a cada publicação. A extensão é publicada como arquivo para carregar manualmente; publicação em lojas e IDs permanentes ainda estão pendentes.

Para compilar e instalar a partir do código local, use `./scripts/install-linux-user.sh`.

Veja [as decisões e pendências da implementação](docs/decisions.md) para revisar o caminho adotado.
O [plano de execução](docs/execution-plan.md) detalha as entregas, dependências e critérios de aceite do desktop Linux e das extensões.

## Princípios

- O formato do cofre continua compatível com GNU `pass`: uma árvore de arquivos `.gpg` cifrados com OpenPGP.
- O terminal é uma interface de primeira classe. O usuário pode continuar usando `pass` e `pass git` sem abrir o Passage.
- O Git privado transporta os arquivos cifrados. Chaves PGP privadas, senhas de desbloqueio e credenciais de acesso ao Git não entram nesse repositório.
- O mesmo cofre deve ser utilizável no desktop, no Android, no terminal e na web, sem depender de um servidor que conheça as senhas em texto claro.
- O projeto deve aproveitar componentes existentes do ecossistema `pass` quando atendam aos requisitos; a interface e as integrações específicas do Passage serão construídas onde houver lacunas.

## Plataformas e formas de acesso

| Forma de acesso | Requisito |
| --- | --- |
| **Desktop** | Aplicativo para consultar e gerenciar o cofre. Linux é a primeira plataforma. |
| **Android** | Aplicativo para consultar e gerenciar o mesmo cofre, com integração ao preenchimento de credenciais do sistema. |
| **Terminal puro** | Uso direto do GNU `pass` e de seus comandos Git, independente dos aplicativos Passage. |
| **Web pura** | Página acessível em um navegador, inclusive em outro computador, capaz de abrir o cofre sem aplicativo ou conector desktop instalado. |

O suporte a iOS é desejável no futuro, se tecnicamente viável, mas está fora do escopo inicial.

## Escopo do MVP

O MVP entrega **aplicativo desktop para Linux e extensões para Brave e Chrome**. Ele resolve o problema original: preencher credenciais nos dois navegadores a partir de um único cofre `pass` local, sem fazer login separadamente em cada extensão.

- O aplicativo desktop abre o cofre existente, permite buscar e consultar logins e controla o acesso das extensões.
- As extensões compartilham uma implementação para navegadores Chromium, são pareadas individualmente com o desktop e permitem revogar cada pareamento.
- A extensão identifica o site aberto, apresenta as entradas correspondentes e preenche usuário e senha após uma ação do usuário.
- O cofre continua utilizável pelo GNU `pass` no terminal. Alterações feitas pelo Passage devem manter a compatibilidade dos arquivos `.gpg`.
- O desktop oferece sincronização com o Git privado e apresenta conflitos ao usuário sem sobrescrever uma entrada silenciosamente.
- O comportamento de bloqueio precisa ser coerente com o acesso direto pelo `pass` e com o cache do `gpg-agent`.

**Integrações existentes durante o MVP:** o GNU `pass` cobre o terminal; o fork [Android Password Store](https://github.com/agrahn/Android-Password-Store) cobre o Android. O Passage não precisa implementar um CLI ou aplicativo Android próprios para validar o MVP.

**Evolução posterior:** cliente web independente, aplicativo Android próprio se o existente deixar lacunas, iOS se viável, passkeys e um modelo de desbloqueio biométrico que proteja a chave também no primeiro acesso após iniciar o sistema. TOTP pode ser avaliado no MVP se o formato existente funcionar sem ampliar o escopo das integrações.

### Desktop e navegadores

- Oferecer busca, consulta e edição dos itens do cofre no aplicativo desktop.
- Preencher logins no Brave e no Chrome por meio de extensões que acessem o cofre local.
- Permitir que os dois navegadores usem o mesmo estado de desbloqueio local, sem exigir um login independente do cofre em cada extensão.
- Parear cada instalação da extensão com o aplicativo ou serviço local e permitir revogar esse acesso.
- Avaliar o desbloqueio pela digital, inclusive no primeiro acesso após iniciar o sistema. O desenho precisa proteger a chave que abre o cofre; a mera confirmação de uma digital não substitui essa proteção.

### Android

- Ler e editar os mesmos arquivos `.gpg` usados pelo `pass` no Linux.
- Integrar senhas ao preenchimento oferecido pelo Android e, para passkeys, ao provedor de credenciais nas versões compatíveis.
- Proteger a chave local usando os recursos de autenticação e armazenamento seguro do dispositivo.
- Sincronizar com o mesmo Git privado, incluindo um caminho claro para configurar a chave PGP e o acesso ao repositório no celular.

### Terminal

- Preservar operações como `pass show`, `pass insert`, `pass edit`, `pass git pull` e `pass git push`.
- Garantir que alterações feitas no terminal apareçam nos demais clientes após a sincronização.
- Documentar a relação entre o bloqueio do aplicativo, o cache do `gpg-agent` e o acesso pelo terminal. A interface não deve afirmar que bloqueou todo o cofre se GPG ainda puder abri-lo.

### Web sem conector desktop

- Permitir autenticação para obter os arquivos `.gpg` cifrados de um repositório Git privado.
- Descriptografar o cofre no navegador com uma implementação OpenPGP compatível; o serviço web não deve receber a chave aberta, a senha da chave ou os itens em texto claro.
- Permitir acesso em um dispositivo novo. O processo de obtenção de uma cópia **cifrada** da chave privada e de sua recuperação precisa ser definido antes da implementação.
- Separar autenticação no serviço/Git da descriptografia do cofre.
- Começar com consulta e cópia de credenciais; definir edição e envio ao Git com controle de conflitos como uma etapa posterior.
- Tratar o código entregue ao navegador como parte da superfície de segurança: distribuição controlada, ausência de scripts de terceiros, proteção contra XSS, política de conteúdo restritiva e sessão curta para material sensível.

## Credenciais

- Logins com usuário e senha.
- Segredos TOTP e geração de códigos de autenticação de dois fatores.
- Passkeys: avaliar a convenção de arquivos do fork [Android Password Store](https://github.com/agrahn/Android-Password-Store/blob/develop/PasskeyStorage.md) e definir como criar, usar e sincronizar essas credenciais no desktop e no Android, mantendo os arquivos utilizáveis pelo `pass`.

## Sincronização e recuperação

- Usar um repositório Git **privado para o cofre**, separado do repositório de código `plu-nz/passage`.
- Aceitar hospedagem em GitHub ou em um servidor Git privado, inclusive no NAS do usuário.
- Detectar alterações simultâneas. Conflitos na mesma entrada não podem ser resolvidos silenciosamente, pois os arquivos `.gpg` não admitem mesclagem textual útil.
- Prever backup e recuperação da chave PGP. Perder o celular, o leitor biométrico ou um dispositivo não pode ser a única causa de perda permanente do cofre.
- Informar que nomes de arquivos, pastas e histórico de commits são visíveis a quem tiver acesso ao repositório Git, embora o conteúdo dos arquivos esteja cifrado.

## Caminho de validação

1. Montar um cofre de teste com `pass` e Git privado, sem credenciais reais.
2. Experimentar Browserpass no Brave e Chrome para identificar o que a extensão própria precisa acrescentar; confirmar que o mesmo cofre abre no Android Password Store.
3. Prototipar o aplicativo Linux e o pareamento com uma extensão Chromium.
4. Validar leitura e escrita interoperáveis com o `pass`, preenchimento nos dois navegadores e sincronização Git com conflitos.
5. Planejar web, Android próprio, passkeys e biometria avançada a partir do MVP validado.

## Decisões técnicas em aberto

- Tecnologia da interface: Solid/Vite com Wails 3 e núcleo Go compartilhado com o host; extensões continuam em TypeScript/WXT.
- Uso direto do Browserpass ou criação de uma extensão e conector próprios.
- Modelo de desbloqueio biométrico no Linux, armazenamento da chave local e interação com `gpg-agent`.
- Como oferecer uma cópia cifrada da chave PGP para acesso web emergencial e como autorizar o serviço web a ler o Git privado.
- Formato interoperável para metadados de site, TOTP e passkeys.
- Política de sincronização, edição concorrente e recuperação de conflitos em cada plataforma.

## Referências

- [GNU pass](https://www.passwordstore.org/)
- [Browserpass](https://github.com/browserpass/browserpass-extension)
- [Android Password Store, fork com passkeys](https://github.com/agrahn/Android-Password-Store)
- [OpenPGP.js](https://docs.openpgpjs.org/)
