; Ganchos del instalador NSIS de CHUPS POS (los llama el script que genera Tauri).

; Acceso directo en el escritorio siempre que se instala (la casilla de la última página lo ofrecía,
; pero en una instalación silenciosa o si se la desmarcaba no quedaba). Al actualizar no se toca.
; El acceso del menú Inicio lo crea el instalador de Tauri.
!macro NSIS_HOOK_POSTINSTALL
  ${If} $UpdateMode <> 1
    Call CreateOrUpdateDesktopShortcut
  ${EndIf}
!macroend

; El desinstalador NO borra los datos (la base de datos ni los respaldos, en %APPDATA%\cl.chups.pos)
; salvo que se marque la casilla "Borrar también mis datos": esa lógica es del instalador de Tauri y
; el texto de la casilla, que avisa que se pierde todo, está en Spanish.nsh. Aquí no hay que hacer nada.
