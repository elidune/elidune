import { forwardRef, useCallback, useRef, useState, type ComponentProps } from 'react';
import { useTranslation } from 'react-i18next';
import { Camera } from 'lucide-react';
import Input from './Input';
import BarcodeCameraScannerModal from './BarcodeCameraScannerModal';
import { getCameraBlockReason, requestCameraStream, stopMediaStream } from '@/utils/cameraSupport';

export type BarcodeScanFieldProps = Omit<ComponentProps<typeof Input>, 'className'> & {
  onCameraScan: (barcode: string) => void;
  scannerTitle?: string;
  wrapperClassName?: string;
  inputClassName?: string;
};

const BarcodeScanField = forwardRef<HTMLInputElement, BarcodeScanFieldProps>(
  (
    {
      onCameraScan,
      scannerTitle,
      wrapperClassName = '',
      inputClassName = '',
      disabled,
      ...inputProps
    },
    ref,
  ) => {
    const { t } = useTranslation();
    const [scannerOpen, setScannerOpen] = useState(false);
    const [initialStream, setInitialStream] = useState<MediaStream | null>(null);
    const initialStreamRef = useRef<MediaStream | null>(null);

    const closeScanner = useCallback(() => {
      stopMediaStream(initialStreamRef.current);
      initialStreamRef.current = null;
      setInitialStream(null);
      setScannerOpen(false);
    }, []);

    const openScanner = useCallback(async () => {
      if (getCameraBlockReason()) {
        initialStreamRef.current = null;
        setInitialStream(null);
        setScannerOpen(true);
        return;
      }

      try {
        const stream = await requestCameraStream();
        initialStreamRef.current = stream;
        setInitialStream(stream);
        setScannerOpen(true);
      } catch {
        initialStreamRef.current = null;
        setInitialStream(null);
        setScannerOpen(true);
      }
    }, []);

    const cameraButton = (
      <button
        type="button"
        disabled={disabled}
        onClick={() => void openScanner()}
        className="rounded-md p-1.5 text-gray-500 hover:bg-gray-100 hover:text-gray-700 focus:outline-none focus-visible:ring-2 focus-visible:ring-amber-500 disabled:opacity-50 dark:text-gray-400 dark:hover:bg-gray-800 dark:hover:text-gray-200 touch-manipulation"
        aria-label={t('common.barcodeScanner.openCamera')}
      >
        <Camera className="h-4 w-4" aria-hidden />
      </button>
    );

    return (
      <>
        <div className={wrapperClassName}>
          <Input
            ref={ref}
            disabled={disabled}
            className={`w-full min-w-0 ${inputClassName}`}
            rightIcon={cameraButton}
            {...inputProps}
            enterKeyHint={inputProps.enterKeyHint ?? 'go'}
          />
        </div>

        <BarcodeCameraScannerModal
          isOpen={scannerOpen}
          onClose={closeScanner}
          onScan={onCameraScan}
          title={scannerTitle ?? t('common.barcodeScanner.title')}
          initialStream={initialStream}
        />
      </>
    );
  },
);

BarcodeScanField.displayName = 'BarcodeScanField';

export default BarcodeScanField;
