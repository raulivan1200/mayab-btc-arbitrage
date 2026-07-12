(() => {
  try {
    const tema = localStorage.getItem("tema") || "dark";
    document.documentElement.setAttribute("data-theme", tema);
    document.documentElement.style.colorScheme = tema;
    const meta = document.getElementById("themeMetaColor");
    if (meta) meta.setAttribute("content", tema === "dark" ? "#0c0e14" : "#f4e9e1");
  } catch {
    // localStorage puede estar deshabilitado; el tema oscuro del HTML es seguro.
  }
})();
