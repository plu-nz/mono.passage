# Passage

Passage é uma proposta de interface multiplataforma para cofres compatíveis com o [GNU `pass`](https://www.passwordstore.org/). O cofre pertence ao usuário: seus arquivos cifrados são sincronizados por um repositório Git privado que pode ser hospedado em um serviço Git ou em infraestrutura própria.

Este documento registra os requisitos discutidos até agora. Ele descreve o produto desejado, não afirma que as funcionalidades já foram implementadas.

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

- Tecnologia da interface: TypeScript com Tauri 2 e código compartilhado com extensões, Flutter, ou outra combinação.
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
