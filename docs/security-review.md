# Revisão de segurança do protótipo

Revisão feita com código e cofre descartável. Em perfis temporários do Brave Origin e Chrome estável, o popup real pareou, listou a entrada e preencheu usuário e senha numa página HTTPS local e no login real do GitHub, sem submeter o formulário. O teste repetiu o clique após navegar a aba e confirmou que o preenchimento foi recusado. Nenhum perfil pessoal ou credencial real foi usado.

## Controles verificados

- O host exige pareamento por instalação, vincula o token ao cofre e à origem da extensão e aceita apenas URLs HTTPS que correspondam ao campo `url` da entrada.
- Revogação e bloqueio persistente impedem novos pedidos; o host verifica novamente a autorização após ler o segredo. Os testes de integração cobrem duas instalações isoladas, domínio semelhante, revogação e bloqueio durante a leitura.
- A extensão obtém a origem da aba pela API do navegador, vincula a lista à aba e à URL, confere ambas novamente antes de preencher e só injeta depois do clique do usuário. Senhas não são salvas em `browser.storage.local`; ali fica somente o token de pareamento.
- O Git recusa árvore suja e divergência antes de sincronizar. Uma ação explícita preserva ambas as versões cifradas em conflitos de `.gpg`, com refs de backup para os commits originais. Conflitos em `.gpg-id` são recusados e abortados sem alterar o HEAD. Novos commits são limitados a `.gpg-id` e `.gpg`; arquivos `.gpg` precisam conter pacotes OpenPGP cifrados.
- A interface não recebe o token do GitHub; a autenticação é delegada ao `gh`. A chave GPG e sua frase secreta permanecem sob controle do GPG e `pinentry`.

## Limites conhecidos

- O preenchimento foi validado em 11 cenários controlados nos perfis temporários do Brave Origin e Chrome estável com o manifesto distribuído: login comum, usuário e senha em etapas separadas, `autocomplete=off`, senhas ou usuários ambíguos, campo escondido ou transparente, campo de nova senha, iframe e página sem login. Os casos ambíguos e sem campo apropriado deixaram os campos vazios. O login real do GitHub passou nos dois navegadores; um segundo formulário externo passou no Chrome estável, mas não carregou no Brave neste ambiente. O Chrome estável foi automatizado em janela normal com `Extensions.loadUnpacked`; ainda falta um teste de instalação pelo seletor de pasta em perfil já usado.
- O bloqueio do host não limpa o cache de `gpg-agent` nem o acesso pelo terminal. Um pedido que já passou pela última verificação pode responder antes de o bloqueio ser aplicado.
- Atualizar ou remover uma entrada verifica o hash do arquivo cifrado, mas há uma janela entre a verificação e o subprocesso `pass`.
- A verificação OpenPGP confirma a presença de pacotes cifrados, mas não autentica o conteúdo nem prova que uma chave privada local consiga abri-lo. Remotos e commits criados por outras ferramentas continuam exigindo confiança no operador.
- O host é um processo do próprio usuário. Outro processo desse usuário pode invocá-lo; o token e a aprovação limitam acesso, mas não protegem contra um processo local que já controle o usuário ou seu chaveiro.
- O pacote tem SHA-256 no mesmo release, ainda sem assinatura independente. O manifesto usa uma chave pública fixa para manter o ID da extensão descompactada em Brave e Chrome; a chave privada não entra no repositório nem no pacote. O instalador registra esse ID no host. Publicação em loja e eventual migração de ID ainda precisam de decisão.
- Nomes de entradas e histórico de commits do Git ficam visíveis para quem acessa o repositório privado. Nunca inclua backup da chave privada nem arquivos em texto claro nesse repositório.

## Critério para publicar

A matriz Brave/Chrome, a instalação e atualização sem checkout local, o clone com remoto privado e os testes de conflito foram executados com dados descartáveis. O workflow do release `v0.2.1` passou; o instalador público instalou desktop, host, extensão `0.2.1` e manifestos para os dois navegadores em um HOME descartável e no usuário local.
