# Fuentes del PDF de la boleta

`Inter-Regular-latin.ttf` e `Inter-Bold-latin.ttf` son Inter 4 (pesos 400 y 700) recortadas a
ASCII, Latin-1 (á, ñ, ü, ¿, ¡, °…) y la puntuación tipográfica más común (€ ‘ ’ “ ” – — … •).
Se incrustan en el binario con `include_bytes!` para que el PDF no dependa de las fuentes del
sistema; recortadas pesan unos 60 KB cada una en vez de 300 KB.

Origen: instancias estáticas de Inter servidas por Google Fonts con el parámetro `text=`
(subconjunto). Licencia: SIL Open Font License 1.1, ver `OFL.txt`. Inter no declara nombres
reservados, así que la versión recortada puede seguir llamándose Inter.

Lo que no esté en el subconjunto se reemplaza antes de dibujar (sin tilde, ASCII o "?"),
igual que en la impresora térmica.
