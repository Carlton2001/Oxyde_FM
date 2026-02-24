import * as pdfjs from 'pdfjs-dist/legacy/build/pdf.mjs';

export const PDF_THUMBNAIL_CACHE = new Map<string, string>();

// No need to set workerSrc if imported directly in some environments, 
// but let's be safe.
pdfjs.GlobalWorkerOptions.workerSrc = new URL(
    'pdfjs-dist/legacy/build/pdf.worker.mjs',
    import.meta.url
).href;

/**
 * Extracts the first page of a PDF as a base64 image string.
 * @param arrayBuffer The PDF file data
 * @param scale The thumbnail scale (e.g., 0.5 or 1.0)
 */
export async function getPdfThumbnail(arrayBuffer: ArrayBuffer, scale: number = 1.0): Promise<string> {
    try {
        const loadingTask = pdfjs.getDocument({ data: arrayBuffer });
        const pdf = await loadingTask.promise;
        const page = await pdf.getPage(1);

        const viewport = page.getViewport({ scale });
        const canvas = document.createElement('canvas');
        const context = canvas.getContext('2d');

        if (!context) {
            throw new Error('Could not get canvas context');
        }

        canvas.height = viewport.height;
        canvas.width = viewport.width;

        const renderContext = {
            canvasContext: context,
            viewport: viewport,
            canvas: canvas
        };

        await page.render(renderContext).promise;

        const dataUrl = canvas.toDataURL();

        // Clean up
        loadingTask.destroy();

        return dataUrl;
    } catch (error) {
        console.error('Error generating PDF thumbnail:', error);
        throw error;
    }
}
