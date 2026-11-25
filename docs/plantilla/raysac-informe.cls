%% raysac-informe.cls — Clase LaTeX común de informes del ecosistema raysac.
%% Compila con tectonic (XeTeX). Uso desde docs/informes/<slug>/<slug>.tex:
%%   \documentclass{../../plantilla/raysac-informe}
\NeedsTeXFormat{LaTeX2e}
\ProvidesClass{raysac-informe}[2026/11/22 v1.0 Clase de informes raysac]

\DeclareOption*{\PassOptionsToClass{\CurrentOption}{article}}
\ProcessOptions\relax
\LoadClass[11pt,a4paper]{article}

% ---------------------------------------------------------------- Paquetes
\RequirePackage{fontspec}
\RequirePackage[spanish,es-noshorthands,es-tabla]{babel}
\RequirePackage[a4paper,margin=2.5cm,headheight=15pt,headsep=14pt,footskip=28pt]{geometry}
\RequirePackage[table]{xcolor}
\RequirePackage{graphicx}
\RequirePackage{booktabs}
\RequirePackage{array}
\RequirePackage{longtable}
\RequirePackage{tabularx}
\RequirePackage{titlesec}
\RequirePackage{enumitem}
\RequirePackage{caption}
\RequirePackage{listings}
\RequirePackage{framed}
\RequirePackage{fancyhdr}
\RequirePackage{lastpage}
\RequirePackage{hyperref}

% ---------------------------------------------------------------- Tipografía
% Tectonic no usa fontconfig: las fuentes se cargan por nombre de archivo del bundle.
\setmainfont{texgyrepagella}[Extension=.otf,UprightFont=*-regular,BoldFont=*-bold,
  ItalicFont=*-italic,BoldItalicFont=*-bolditalic]
\setsansfont{texgyreheros}[Extension=.otf,UprightFont=*-regular,BoldFont=*-bold,
  ItalicFont=*-italic,BoldItalicFont=*-bolditalic]
\setmonofont{lmmono10-regular}[Extension=.otf,ItalicFont=lmmono10-italic,
  BoldFont=lmmonolt10-bold,BoldItalicFont=lmmonolt10-boldoblique,Scale=0.9]
\linespread{1.08}
\setlength{\parindent}{0pt}
\setlength{\parskip}{0.55em plus 0.15em minus 0.1em}
\setlist{itemsep=0.2em,topsep=0.3em,parsep=0pt}
\renewcommand{\arraystretch}{1.25}
\setcounter{tocdepth}{3}
\setcounter{secnumdepth}{3}

% Nombres en español (babel con es-tabla ya usa "Tabla"; se refuerzan)
\addto\captionsspanish{%
  \renewcommand{\tablename}{Tabla}%
  \renewcommand{\listtablename}{Índice de tablas}%
  \renewcommand{\listfigurename}{Índice de figuras}%
  \renewcommand{\contentsname}{Índice}%
  \renewcommand{\figurename}{Figura}%
}

% ---------------------------------------------------------------- Colores
\definecolor{raysacdark}{HTML}{1F2A44}
\definecolor{raysacaccent}{HTML}{44546A}
\definecolor{raysacrule}{HTML}{8497B0}
\definecolor{raysacrow}{HTML}{EEF1F5}
\definecolor{raysaclink}{HTML}{1F4E79}
\definecolor{raysacnotabg}{HTML}{F2F5F9}
\definecolor{raysacavisobg}{HTML}{FFF6E5}
\definecolor{raysacavisoborde}{HTML}{B7791F}
\definecolor{lstbg}{HTML}{F7F8FA}
\definecolor{lstrule}{HTML}{C9D1DC}
\definecolor{lstkw}{HTML}{1F4E79}
\definecolor{lsttype}{HTML}{7A3E9D}
\definecolor{lstcomment}{HTML}{6B7785}
\definecolor{lststring}{HTML}{2E7D32}

% ---------------------------------------------------------------- Metadatos
\newcommand{\raysac@producto}{}
\newcommand{\raysac@titulo}{}
\newcommand{\raysac@titulocorto}{}
\newcommand{\raysac@version}{}
\newcommand{\raysac@fecha}{\today}
\newcommand{\raysac@clasificacion}{Uso interno}
\newcommand{\raysac@autor}{raysac}

\newcommand{\producto}[1]{\renewcommand{\raysac@producto}{#1}}
\newcommand{\titulo}[1]{\renewcommand{\raysac@titulo}{#1}%
  \ifx\raysac@titulocorto\@empty\renewcommand{\raysac@titulocorto}{#1}\fi}
\newcommand{\titulocorto}[1]{\renewcommand{\raysac@titulocorto}{#1}}
\providecommand{\version}[1]{}
\renewcommand{\version}[1]{\renewcommand{\raysac@version}{#1}}
\newcommand{\fecha}[1]{\renewcommand{\raysac@fecha}{#1}}
\newcommand{\clasificacion}[1]{\renewcommand{\raysac@clasificacion}{#1}}
\newcommand{\autor}[1]{\renewcommand{\raysac@autor}{#1}}

\hypersetup{%
  colorlinks=true,
  linkcolor=raysaclink,
  urlcolor=raysaclink,
  citecolor=raysaclink,
  bookmarksnumbered=true,
  pdfcreator={raysac-informe}}
\AtBeginDocument{%
  \hypersetup{%
    pdftitle={\raysac@titulo},
    pdfauthor={\raysac@autor},
    pdfsubject={\raysac@producto}}%
}

% ---------------------------------------------------------------- Títulos
\titleformat{\section}
  {\sffamily\bfseries\Large\color{raysacdark}}{\thesection}{0.8em}{}
  [{\color{raysacrule}\titlerule[0.6pt]}]
\titleformat{\subsection}
  {\sffamily\bfseries\large\color{raysacdark}}{\thesubsection}{0.8em}{}
\titleformat{\subsubsection}
  {\sffamily\bfseries\normalsize\color{raysacaccent}}{\thesubsubsection}{0.8em}{}
\titlespacing*{\section}{0pt}{2.4ex plus 0.6ex minus 0.2ex}{1.4ex plus 0.2ex}
\titlespacing*{\subsection}{0pt}{2.0ex plus 0.5ex minus 0.2ex}{1.0ex plus 0.2ex}
\titlespacing*{\subsubsection}{0pt}{1.6ex plus 0.4ex minus 0.2ex}{0.8ex plus 0.2ex}

% ---------------------------------------------------------------- Leyendas
\captionsetup{font={small},labelfont={bf,sf},labelsep=period,skip=6pt}

% ---------------------------------------------------------------- Encabezado / pie
\pagestyle{fancy}
\fancyhf{}
\fancyhead[L]{\sffamily\small\color{raysacaccent}\raysac@producto}
\fancyhead[R]{\sffamily\small\color{raysacaccent}\raysac@titulocorto}
\fancyfoot[L]{\sffamily\small\color{raysacaccent}\raysac@clasificacion}
\fancyfoot[C]{\sffamily\small\color{raysacaccent}\ifx\raysac@version\@empty\else v\raysac@version\fi}
\fancyfoot[R]{\sffamily\small\color{raysacaccent}Página \thepage\ de \pageref*{LastPage}}
\renewcommand{\headrulewidth}{0.4pt}
\renewcommand{\footrulewidth}{0.4pt}
\renewcommand{\headrule}{{\color{raysacrule}\hrule width\headwidth height\headrulewidth}}
\renewcommand{\footrule}{{\color{raysacrule}\hrule width\headwidth height\footrulewidth}}
\fancypagestyle{plain}{\fancyhf{}%
  \fancyfoot[R]{\sffamily\small\color{raysacaccent}Página \thepage\ de \pageref*{LastPage}}%
  \renewcommand{\headrulewidth}{0pt}}

% ---------------------------------------------------------------- Figuras
% Las figuras se buscan en docs/diagramas/png (compilación desde docs/informes/<slug>/).
\graphicspath{{../../diagramas/png/}{../diagramas/png/}{./}}

% ---------------------------------------------------------------- Portada
\newcommand{\portada}{%
  \begingroup
  \thispagestyle{empty}%
  \raggedright
  \vspace*{1.5cm}%
  {\sffamily\bfseries\fontsize{13}{16}\selectfont\color{raysacaccent}\MakeUppercase{\raysac@producto}\par}%
  \vspace{6pt}%
  {\color{raysacdark}\rule{\textwidth}{1.4pt}\par}%
  \vspace{3.2cm}%
  {\sffamily\bfseries\fontsize{30}{36}\selectfont\color{raysacdark}\raysac@titulo\par}%
  \vspace{1.2cm}%
  {\color{raysacrule}\rule{4cm}{1.2pt}\par}%
  \vfill
  {\sffamily\color{raysacdark}%
  \renewcommand{\arraystretch}{1.6}%
  \begin{tabular}{@{}p{3.6cm}p{10cm}@{}}
    \bfseries Producto      & \raysac@producto \\
    \bfseries Versión       & \raysac@version \\
    \bfseries Fecha         & \raysac@fecha \\
    \bfseries Autor         & \raysac@autor \\
    \bfseries Clasificación & \raysac@clasificacion \\
  \end{tabular}\par}%
  \vspace{1.2cm}%
  {\color{raysacdark}\rule{\textwidth}{0.6pt}\par}%
  \vspace{4pt}%
  {\sffamily\small\color{raysacaccent}\raysac@autor\hfill\raysac@fecha\par}%
  \newpage
  \endgroup
}

% ---------------------------------------------------------------- Control de versiones
% \begin{controlversiones}
%   \cvfila{1.0}{2026-11-22}{raysac}{Versión inicial.}
% \end{controlversiones}
\newcommand{\cvfila}[4]{#1 & #2 & #3 & #4 \\}
\newenvironment{controlversiones}{%
  \section*{Control de versiones del documento}%
  \par\vspace{0.4em}%
  \begin{longtable}{@{}>{\raggedright\arraybackslash}p{1.8cm}%
                      >{\raggedright\arraybackslash}p{2.6cm}%
                      >{\raggedright\arraybackslash}p{2.8cm}%
                      >{\raggedright\arraybackslash}p{\dimexpr\textwidth-7.2cm-8\tabcolsep\relax}@{}}
  \toprule
  \rowcolor{raysacrow}\sffamily\bfseries Versión & \sffamily\bfseries Fecha &
    \sffamily\bfseries Autor & \sffamily\bfseries Descripción del cambio \\
  \midrule
  \endhead
  \bottomrule
  \endlastfoot
}{%
  \end{longtable}%
  % longtable avanza el contador de tablas aunque no haya \caption: se compensa.
  \addtocounter{table}{-1}%
}

% ---------------------------------------------------------------- Índices
\newcommand{\preliminares}{%
  \tableofcontents
  \bigskip
  \listoffigures
  \bigskip
  \listoftables
  \clearpage
}

% ---------------------------------------------------------------- Avisos
\newenvironment{raysac@caja}[3]{%
  \def\FrameCommand{\fcolorbox{#1}{#2}}%
  \setlength{\fboxsep}{9pt}\setlength{\fboxrule}{0.8pt}%
  \MakeFramed{\advance\hsize-\width\advance\hsize-2\fboxsep \FrameRestore}%
  {\sffamily\bfseries\color{#1}#3\par}\smallskip\small
}{\endMakeFramed\par\medskip}

\newenvironment{avisolegal}{\begin{raysac@caja}{raysacavisoborde}{raysacavisobg}{Aviso legal}}%
  {\end{raysac@caja}}
\newenvironment{nota}{\begin{raysac@caja}{raysacaccent}{raysacnotabg}{Nota}}%
  {\end{raysac@caja}}

\newcommand{\avisolegalestandar}{%
  \begin{avisolegal}
    Este documento es de carácter técnico e informativo y \textbf{no constituye asesoría legal}.
    Las referencias normativas deben ser validadas por un abogado antes de tomar decisiones
    jurídicas o de cumplimiento. Las cifras, tasas y topes citados indican su fuente y fecha de
    consulta; pueden haber cambiado desde entonces.
  \end{avisolegal}%
}

% ---------------------------------------------------------------- Glosario
\newenvironment{glosario}{%
  \begin{description}[style=multiline,leftmargin=3.6cm,labelwidth=3.4cm,
    font={\sffamily\bfseries\color{raysacdark}},itemsep=0.35em]%
}{\end{description}}
\newcommand{\termino}[2]{\item[#1] #2}

% ---------------------------------------------------------------- Listings
\lstdefinelanguage{Rust}{%
  keywords={as,async,await,break,const,continue,crate,dyn,else,enum,extern,false,fn,for,if,impl,
    in,let,loop,match,mod,move,mut,pub,ref,return,self,static,struct,super,trait,true,type,
    unsafe,use,where,while},
  keywords=[2]{bool,char,f32,f64,i8,i16,i32,i64,i128,isize,u8,u16,u32,u64,u128,usize,str,
    String,Vec,Option,Result,Some,None,Ok,Err,Box,Self,HashMap},
  morecomment=[l]{//},
  morecomment=[s]{/*}{*/},
  morestring=[b]",
  sensitive=true}
\lstdefinelanguage{TypeScript}{%
  keywords={abstract,as,async,await,break,case,catch,class,const,continue,default,delete,do,else,
    enum,export,extends,false,finally,for,from,function,if,implements,import,in,instanceof,
    interface,let,new,null,of,private,protected,public,readonly,return,static,super,switch,this,
    throw,true,try,type,typeof,undefined,var,void,while,yield},
  keywords=[2]{any,boolean,number,string,unknown,never,Promise,Array,Record,Partial,Map,Set},
  morecomment=[l]{//},
  morecomment=[s]{/*}{*/},
  morestring=[b]",
  morestring=[b]',
  morestring=[b]`,
  sensitive=true}
\lstdefinelanguage{JSON}{%
  keywords={true,false,null},
  morestring=[b]",
  sensitive=true}

\lstdefinestyle{raysac}{%
  basicstyle=\ttfamily\footnotesize,
  keywordstyle=\color{lstkw}\bfseries,
  keywordstyle=[2]\color{lsttype},
  commentstyle=\color{lstcomment}\itshape,
  stringstyle=\color{lststring},
  numbers=left,
  numberstyle=\tiny\sffamily\color{lstcomment},
  numbersep=8pt,
  backgroundcolor=\color{lstbg},
  frame=single,
  rulecolor=\color{lstrule},
  framesep=5pt,
  xleftmargin=1.6em,
  framexleftmargin=1.6em,
  breaklines=true,
  breakindent=0pt,
  postbreak=\mbox{\textcolor{raysacrule}{$\hookrightarrow$}\space},
  showstringspaces=false,
  keepspaces=true,
  columns=fullflexible,
  tabsize=2,
  aboveskip=0.8em,
  belowskip=0.6em,
  captionpos=b}
\lstset{style=raysac}
\renewcommand{\lstlistingname}{Listado}
\renewcommand{\lstlistlistingname}{Índice de listados}

\endinput
