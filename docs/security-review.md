# Revisão de segurança do protótipo

Revisão feita com código e cofre descartável. O release público ainda não inclui este conjunto. Em perfis temporários do Brave Origin e Chrome for Testing, o popup real pareou, listou a entrada e preencheu usuário e senha numa página HTTPS local. O teste repetiu o clique após navegar a aba para outra URL da mesma origem e confirmou que o preenchimento foi recusado. Nenhum perfil pessoal ou credencial real foi usado.

## Controles verificados

- O host exige pareamento por instalação, vincula o token ao cofre e à origem da extensão e aceita apenas URLs HTTPS que correspondam ao campo `url` da entrada.
- Revogação e bloqueio persistente impedem novos pedidos; o host verifica novamente a autorização após ler o segredo. Os testes de integração cobrem duas instalações isoladas, domínio semelhante, revogação e bloqueio durante a leitura.
- A extensão obtém a origem da aba pela API do navegador, vincula a lista à aba e à URL, confere ambas novamente antes de preencher e só injeta depois do clique do usuário. Senhas não são salvas em `browser.storage.local`; ali fica somente o token de pareamento.
- O Git recusa árvore suja e divergência antes de sincronizar. O teste com dois clones confirma que uma divergência não substitui o HEAD local.
- A interface não recebe o token do GitHub; a autenticação é delegada ao `gh`. A chave GPG e sua frase secreta permanecem sob controle do GPG e `pinentry`.

## Limites para validação antes de release

- O preenchimento foi validado em 11 cenários controlados nos perfis temporários do Brave Origin e Chrome for Testing com o manifesto distribuído: login comum, usuário e senha em etapas separadas, `autocomplete=off`, senhas ou usuários ambíguos, campo escondido ou transparente, campo de nova senha, iframe e página sem login. Os casos ambíguos e sem campo apropriado deixaram os campos vazios. Ainda faltam sites reais e instalação manual no Chrome estável. O Chrome estável instalado aqui não aceita `--load-extension`; o Chrome for Testing permitiu a automação. O seletor de pasta de `chrome://extensions` não foi exposto à automação neste ambiente, mesmo com perfil descartável.
- O bloqueio do host não limpa o cache de `gpg-agent` nem o acesso pelo terminal. Um pedido que já passou pela última verificação pode responder antes de o bloqueio ser aplicado.
- Atualizar ou remover uma entrada verifica o hash do arquivo cifrado, mas há uma janela entre a verificação e o subprocesso `pass`.
- O host é um processo do próprio usuário. Outro processo desse usuário pode invocá-lo; o token e a aprovação limitam acesso, mas não protegem contra um processo local que já controle o usuário ou seu chaveiro.
- O pacote tem SHA-256 no mesmo release, ainda sem assinatura independente. O manifesto usa uma chave pública fixa para manter o ID da extensão descompactada em Brave e Chrome; a chave privada não entra no repositório nem no pacote. O instalador registra esse ID no host. Publicação em loja e eventual migração de ID ainda precisam de decisão.
- Nomes de entradas e histórico de commits do Git ficam visíveis para quem acessa o repositório privado. Nunca inclua backup da chave privada nem arquivos em texto claro nesse repositório.

## Critério para publicar

Concluir a matriz Brave/Chrome com cofre de teste, verificar instalação e atualização sem checkout local, e revisar os limites acima. Registrar o resultado observado antes de disparar o workflow de release.
