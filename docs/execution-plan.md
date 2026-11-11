# Plano de execução: desktop Linux e extensões

Este plano transforma os requisitos do [README](../README.md) em entregas verificáveis. Cobre o aplicativo Linux, as extensões Brave/Chrome, a interoperabilidade com GNU `pass` no terminal, Git, segurança e distribuição. Android, cliente web e iOS ficam fora deste plano. Estimativas são esforço aproximado de uma pessoa familiarizada com Rust, TypeScript e extensões Chromium, não prazos prometidos.

## Resultado esperado

Uma instalação local do Passage usa um cofre GNU `pass` existente. O usuário desbloqueia sua chave GPG pelo mecanismo do sistema uma vez; Brave e Chrome podem pedir preenchimento ao mesmo serviço local, sem uma senha independente por extensão. Cada instalação da extensão precisa ser autorizada e pode ser revogada. O usuário também pode ler e editar entradas no desktop, usar `pass` no terminal e sincronizar arquivos cifrados com um Git privado, com conflitos explícitos.

## Regras de implementação

- Preservar o formato `.gpg`, a estrutura de diretórios, `.gpg-id` e o comportamento de `pass`; não implementar criptografia OpenPGP própria no MVP.
- Tratar o processo local Rust como autoridade para acesso ao cofre. A interface Tauri e as extensões são clientes; compartilhar regras de domínio, formato de entrada e protocolo em código testável.
- Não considerar a chave GPG bloqueada só porque a interface escondeu a senha: o cache do `gpg-agent` é independente.
- Não guardar credenciais abertas em logs, arquivos temporários, Git, armazenamento da extensão ou telemetria.
- Testar integração real com Brave e Chrome e um cofre descartável antes de marcar uma etapa como concluída.

## Marco 0 — Ambiente e referência funcional (1–2 dias)

1. Criar um cofre descartável com chave GPG de teste, entradas em subpastas, múltiplos destinatários via `.gpg-id`, URLs, usuário, TOTP e Git remoto privado de teste. Não usar credenciais reais.
2. Exercitar `pass show`, `insert`, `edit`, `rm`, `git pull` e `git push`; registrar a convenção inicial de campos e o comportamento esperado do `gpg-agent`.
3. Rodar `pnpm check`, build do desktop e da extensão, testes Rust e instalação pelo release atual. Registrar dependências de execução e falhas reproduzíveis.
4. Validar o app desktop contra o cofre de teste: `PASSWORD_STORE_DIR`, ausência de `pass`, senha incorreta no GPG, atualização após alteração externa e arquivos simbólicos.

**Aceite:** roteiro de testes reproduzível e cofre descartável; leitura no desktop e pelo terminal confirmada; limitações atuais documentadas. Esta etapa estabelece a referência para as seguintes.

## Marco 1 — Núcleo do cofre e modelo de entrada (3–5 dias)

1. Extrair do comando Tauri a lógica Rust de localizar, listar, ler, criar, atualizar e remover entradas. Validar IDs, caminho canônico, symlinks, nomes vazios, erros de subprocesso e resultado UTF-8.
2. Definir uma convenção de texto interoperável: primeira linha como senha; campos `login`, `url` e `otpauth` nas linhas seguintes. Preservar linhas desconhecidas e a ordem existente ao editar. Documentar como um item antigo sem metadados aparece.
3. Especificar normalização e correspondência de origem: esquema, host, subdomínio e porta; evitar correspondência por sufixo inseguro (por exemplo `example.com.evil.test`). O domínio da aba será obtido pela extensão, jamais aceito de conteúdo da página.
4. Implementar operações via `pass` e testar round-trip com `pass show`, incluindo edição pelo terminal seguida de atualização no desktop. Tratar entradas alteradas externamente antes da gravação.
5. Definir erros estáveis para cofre ausente, chave indisponível, item inválido, conflito de edição e Git, sem colocar segredo nas mensagens.

**Aceite:** criar, editar e remover pelo núcleo mantém arquivos legíveis por GNU `pass`; campos desconhecidos sobrevivem à edição; entradas maliciosas não escapam do cofre. Testes automatizados usam somente cofre descartável.

## Marco 2 — Serviço local e protocolo da extensão (4–7 dias)

1. Implementar executável host de Native Messaging em Rust, com mensagens JSON enquadradas pelo protocolo Chromium, limites de tamanho, versão do protocolo, timeouts e encerramento limpo. Reutilizar o núcleo do Marco 1 sem depender de uma janela Tauri aberta.
2. Decidir e fixar o modelo de autorização: cada instalação da extensão se identifica e solicita pareamento no desktop; autorização explícita do usuário; credencial local por instalação protegida no perfil do usuário; listagem e revogação. Um ID de extensão permitido no manifesto do host é necessário, mas não basta como autorização da instância.
3. Definir pedidos mínimos `status`, `pair`, `listForOrigin` e `readForOrigin`. Vincular cada pedido sensível à origem da aba e ao escopo autorizado; validar versão, identidade, estado do pareamento e tamanho do pedido no host.
4. Evitar enumeração indiscriminada de entradas pelo navegador. O host retorna apenas candidatas para a origem solicitada e só libera senha após ação explícita na extensão. Não enviar segredos em resposta a `ping` ou enquanto o popup apenas abre.
5. Registrar a decisão sobre processo: host Native Messaging iniciado sob demanda para cada navegador, com autorizações persistidas e GPG compartilhado pelo sistema. Se uma janela Tauri for necessária para aprovar pareamento, expor apenas esse fluxo local.
6. Fazer testes de protocolo com entrada truncada, JSON inválido, versão incompatível, origem inválida, identidade não pareada e revogação durante uma sessão.

**Aceite:** Brave e Chrome conseguem chamar o host instalado; uma instalação não pareada não lê entradas; revogação impede pedidos posteriores; fechar o app Tauri não torna a integração inoperante. Nunca há senha na saída de erro ou logs.

## Marco 3 — Extensões Brave e Chrome e preenchimento (4–7 dias)

1. Distribuir a mesma base WXT/Manifest V3 para os dois navegadores. Fixar IDs de extensão conforme o canal de distribuição e registrar cada ID nos manifestos do host, sem permissões de origem amplas por padrão.
2. No popup, obter URL e identidade da aba ativa pela API da extensão, solicitar candidatas ao host, exibir título/usuário e permitir escolher uma entrada. Pedir permissão de site apenas quando necessária.
3. Implementar script de preenchimento de usuário/senha após clique. Verificar aba, frame e origem imediatamente antes da injeção; recusar páginas privilegiadas e mudança de navegação. Não preencher automaticamente nem enviar segredo ao DOM sem ação do usuário.
4. Suportar formulários usuais, login em duas etapas e campos com eventos de input/change. Tratar ausência ou ambiguidade de campos sem preencher um destino arbitrário; informar o usuário quando for preciso copiar manualmente.
5. Limpar referências ao segredo no popup/background após a operação, dentro dos limites do runtime JavaScript. Não persistir senhas em `storage`, histórico ou console.
6. Testar pareamento, seleção e preenchimento em Brave e Chrome com perfis limpos e páginas de teste controladas. Incluir tentativa de site semelhante, iframe de outra origem e navegação entre seleção e preenchimento.

**Aceite:** uma entrada do mesmo cofre preenche corretamente nos dois navegadores após uma única autenticação GPG aplicável; cada extensão é pareada/revogada de modo independente; um site sem correspondência não recebe segredo.

## Marco 4 — Desktop completo, Git e terminal (5–8 dias)

1. Adicionar ao desktop criação, edição e exclusão com confirmação adequada, preservação de campos desconhecidos e atualização após mudanças externas. Exibir usuário, URLs e metadados sem revelar a senha por padrão.
2. Implementar status Git, pull e push para remoto configurado pelo usuário, sem assumir GitHub. Exibir branch, remoto, alterações locais e erros de credencial de forma útil, sem ecoar tokens.
3. Para sincronizar, detectar trabalho local não salvo e divergência remota. Em conflito na mesma entrada `.gpg`, interromper a operação e oferecer escolha explícita entre versões ou cópia de segurança; nunca substituir silenciosamente. Não tentar mesclar ciphertext como texto.
4. Garantir que `pass git` e alterações feitas diretamente no repositório continuem funcionando. Atualizar a UI quando o cofre mudar no disco ou oferecer atualização clara e confiável.
5. Documentar backup/recuperação de chave GPG e acesso ao remoto; destacar que nomes de entradas e histórico Git ficam visíveis a quem acessa o repositório.

**Aceite:** criar e editar no Passage reflete no `pass` e vice-versa; sincronização com GitHub e servidor Git genérico funciona com autenticação já configurada no sistema; conflito reproduzido não perde nenhuma das duas versões.

## Marco 5 — Bloqueio, TOTP e biometria (3–6 dias para bloqueio/TOTP; biometria depende de protótipo)

1. Definir o que o botão “bloquear” controla: limpar segredos da UI e suspender pedidos sensíveis das extensões imediatamente. Exibir separadamente o estado do `gpg-agent`, pois este pode continuar liberando `pass show` no terminal.
2. Escolher política de sessão e reautorização para extensão, incluindo expiração, bloqueio de tela e revogação. Verificar as limitações reais do cache de GPG antes de prometer um desbloqueio único em todas as circunstâncias.
3. Suportar TOTP por convenção compatível com `pass-otp` quando presente; gerar códigos apenas em memória e testar interoperabilidade. Não alterar os arquivos apenas para mostrar o código.
4. Prototipar biometria Linux com a chave GPG de teste. A digital deve liberar material criptográfico protegido ou um mecanismo do sistema que o libere; uma confirmação biométrica sobre uma chave já aberta não satisfaz desbloqueio após reiniciar. Avaliar integração com PAM/FIDO2 ou armazenamento seguro disponível por distribuição e fallback de senha/chave de recuperação.
5. Só assumir biometria como requisito entregue após demonstrar o primeiro acesso depois de boot, recuperação sem leitor e comportamento coerente com `gpg-agent` e `pass` no terminal. Se o protótipo não alcançar isso, documentar a limitação e manter fora do release estável.

**Aceite:** bloqueio do Passage corta acesso das extensões mesmo com GPG em cache; UI descreve corretamente o que o bloqueio cobre; TOTP coincide com `pass-otp`; biometria tem prova de ponta a ponta ou permanece explicitamente experimental.

## Marco 6 — Distribuição, segurança e release (3–5 dias)

1. Empacotar app Tauri, host Native Messaging, manifestos para Brave/Chrome e extensões como conjunto versionado. O instalador remoto deve instalar/atualizar cada peça e oferecer remoção limpa, sem exigir clone do repositório.
2. Automatizar build, tipos, testes Rust/TypeScript, artefatos e checksums no CI. Fixar versões de dependências e considerar assinatura dos artefatos antes de recomendar `curl | sh` como caminho principal de instalação.
3. Revisar permissões da extensão, superfícies de injeção, protocolo, armazenamento local de pareamentos, subprocessos, proteção contra paths arbitrários e exposição acidental de segredos. Fazer revisão manual com um cofre descartável.
4. Executar matriz final: Brave e Chrome em perfis novos e já usados; cofre local com e sem Git; ausência de `pass`; GPG com e sem cache; lock/revogação; instalador sem repo clonado; atualização de uma versão anterior.
5. Atualizar README, página de instalação e decisões com funcionalidades efetivamente disponíveis e limitações conhecidas. Publicar release apenas após a matriz passar.

**Aceite:** instalar em Linux x86_64 por `passage.plu.nz/install.sh` deixa desktop e conexão com os dois navegadores utilizáveis; artefatos são reproduzíveis a partir do commit; README não promete funções pendentes.

## Ordem, dependências e entregas

| Sequência | Depende de | Entrega revisável | Esforço |
| --- | --- | --- | --- |
| 0. Referência funcional | Código atual | Cofre de teste e roteiro | 1–2 dias |
| 1. Núcleo e formato | 0 | CRUD compatível com `pass` | 3–5 dias |
| 2. Host e pareamento | 1 | Protocolo local autorizado | 4–7 dias |
| 3. Extensões | 2 | Preenchimento Brave/Chrome | 4–7 dias |
| 4. Desktop e Git | 1; integrações revisadas com 2–3 | Edição e sincronização sem perda silenciosa | 5–8 dias |
| 5. Bloqueio e TOTP | 2–4 | Política de sessão e códigos; protótipo biométrico separado | 3–6 dias + pesquisa |
| 6. Release | 0–5 | Instalador completo, CI e documentação | 3–5 dias |

O caminho crítico para validar a ideia é **0 → 1 → 2 → 3**. Após esse fluxo funcionar nos dois navegadores, avançar para Git, bloqueio e distribuição estável. Estimativa total do trabalho definido: **23–40 dias úteis**, mais a investigação biométrica, que não tem estimativa confiável antes do protótipo.

## Decisões a registrar ao longo da execução

- Convenção final dos campos `login`, `url` e `otpauth`; comportamento com entradas antigas e múltiplas URLs.
- IDs estáveis e canal de distribuição das extensões; formato e local de instalação do host em cada navegador.
- Identidade de instalação, armazenamento de autorização e interação de pareamento com a janela desktop.
- Política de lock em relação a `gpg-agent`, sessão do host e desbloqueio inicial.
- Estratégia de Git para diretório sujo, conflito e recuperação; escolha de biblioteca ou CLI Git.
- Compatibilidade de TOTP com `pass-otp` e viabilidade de biometria após boot.

Cada marco deve atualizar [decisions.md](decisions.md), README e critérios de aceite com o resultado observado, incluindo desvios de segurança ou escopo.
