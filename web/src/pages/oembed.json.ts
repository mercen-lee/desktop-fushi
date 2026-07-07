const siteUrl = "https://desktopfushi.mercen.net";
const embedUrl = `${siteUrl}/embed`;
const thumbnailUrl = `${siteUrl}/desktop-fushi-og.png`;
const iframeHtml = `<iframe src="${embedUrl}" width="800" height="600" title="Desktop Fushi" loading="lazy" allow="webgpu; fullscreen" style="display:block;width:100%;max-width:800px;aspect-ratio:4 / 3;border:0;background:#f6f2e8;"></iframe>`;

export function GET() {
  return new Response(
    JSON.stringify(
      {
        version: "1.0",
        type: "rich",
        provider_name: "Desktop Fushi",
        provider_url: siteUrl,
        title: "Desktop Fushi",
        html: iframeHtml,
        width: 800,
        height: 600,
        thumbnail_url: thumbnailUrl,
        thumbnail_width: 1200,
        thumbnail_height: 630,
        cache_age: 86400,
      },
      null,
      2,
    ),
    {
      headers: {
        "Content-Type": "application/json; charset=utf-8",
      },
    },
  );
}
